package com.hiqnet.pos.safdocument

import android.app.Activity
import android.content.ContentResolver
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.io.IOException
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors

@InvokeArg
class ExportDocumentArgs {
    var displayName: String? = null
    var sourcePath: String? = null
}

@InvokeArg
class ImportDocumentArgs {
    var targetPath: String? = null
}

/**
 * Transporte mínimo Android Storage Access Framework.
 *
 * Las rutas proceden exclusivamente del backend Rust mediante el puente de
 * plugin; React no las construye. El plugin no interpreta ni valida SQLite.
 *
 * Reglas de concurrencia:
 * - una sola operación activa, desde la apertura del selector hasta que la
 *   copia termina;
 * - las copias se ejecutan fuera del hilo principal, en un executor compartido
 *   con hilos daemon, para no acumular hilos al recrear la Activity;
 * - el `ContentResolver` se toma del contexto de aplicación, que sobrevive a la
 *   Activity que abrió el selector;
 * - ningún callback intenta responder a un proceso que ya no escucha: los
 *   errores de respuesta se ignoran después del desmontaje.
 */
@TauriPlugin
class SafDocumentPlugin(private val activity: Activity) : Plugin(activity) {
    private val lock = Any()
    private val gate = OperationGate()
    private val pendingExports = mutableMapOf<Long, File>()
    private val pendingImports = mutableMapOf<Long, File>()

    private val resolver: ContentResolver
        get() = activity.applicationContext.contentResolver

    @Command
    fun exportDocument(invoke: Invoke) {
        val args = invoke.parseArgs(ExportDocumentArgs::class.java)
        val source = args.sourcePath?.let(::File)
        val name = args.displayName?.trim()
        if (source == null || !source.isFile || name.isNullOrEmpty() ||
            name.contains('/') || name.contains('\\')
        ) {
            invoke.reject("No fue posible preparar el respaldo para exportarlo.", "invalid_request")
            return
        }
        if (source.length() > DocumentIo.MAX_DOCUMENT_BYTES) {
            invoke.reject("El respaldo es demasiado grande para exportarlo.", "too_large")
            return
        }
        if (!gate.begin()) {
            invoke.reject("Ya hay una operación de documentos en curso.", "busy")
            return
        }
        synchronized(lock) { pendingExports[invoke.id] = source }
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("application/x-sqlite3")
            .putExtra(Intent.EXTRA_TITLE, name)
            .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        try {
            startActivityForResult(invoke, intent, "onExportDocumentResult")
        } catch (error: Exception) {
            synchronized(lock) { pendingExports.remove(invoke.id) }
            gate.end()
            invoke.reject("No fue posible abrir el selector de documentos.", "picker_failed", error)
        }
    }

    @ActivityCallback
    fun onExportDocumentResult(invoke: Invoke, result: ActivityResult) {
        val source = synchronized(lock) { pendingExports.remove(invoke.id) }
        val destination = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || destination == null || source == null) {
            gate.end()
            respond { invoke.resolve(result("cancelled", null)) }
            return
        }
        executor.execute {
            try {
                val copied = copyToDocument(source, destination)
                respond { invoke.resolve(result("completed", copied)) }
            } catch (error: Exception) {
                // No se publica el documento parcial: si el proveedor lo permite,
                // se elimina el archivo que acabamos de crear.
                deleteDocumentQuietly(destination)
                respond {
                    invoke.reject(
                        "No fue posible escribir el respaldo en el destino seleccionado.",
                        "write_failed",
                        error,
                    )
                }
            } finally {
                gate.end()
            }
        }
    }

    @Command
    fun importDocument(invoke: Invoke) {
        val args = invoke.parseArgs(ImportDocumentArgs::class.java)
        val target = args.targetPath?.let(::File)
        if (target == null || target.exists()) {
            invoke.reject("No fue posible preparar la importación del respaldo.", "invalid_request")
            return
        }
        if (!gate.begin()) {
            invoke.reject("Ya hay una operación de documentos en curso.", "busy")
            return
        }
        synchronized(lock) { pendingImports[invoke.id] = target }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        try {
            startActivityForResult(invoke, intent, "onImportDocumentResult")
        } catch (error: Exception) {
            synchronized(lock) { pendingImports.remove(invoke.id) }
            gate.end()
            invoke.reject("No fue posible abrir el selector de documentos.", "picker_failed", error)
        }
    }

    @ActivityCallback
    fun onImportDocumentResult(invoke: Invoke, result: ActivityResult) {
        val target = synchronized(lock) { pendingImports.remove(invoke.id) }
        val source = result.data?.data
        if (result.resultCode != Activity.RESULT_OK || source == null || target == null) {
            gate.end()
            respond { invoke.resolve(result("cancelled", null)) }
            return
        }
        executor.execute {
            try {
                val copied = copyFromDocument(source, target)
                respond { invoke.resolve(result("completed", copied)) }
            } catch (error: Exception) {
                target.delete()
                respond {
                    invoke.reject(
                        "No fue posible leer el respaldo seleccionado.",
                        "read_failed",
                        error,
                    )
                }
            } finally {
                gate.end()
            }
        }
    }

    private fun copyToDocument(source: File, destination: Uri): Long {
        resolver.openOutputStream(destination, "w")?.use { output ->
            FileInputStream(source).use { input ->
                return DocumentIo.copyBounded(input, output)
            }
        }
        throw IOException("El proveedor no abrió un flujo de escritura.")
    }

    private fun copyFromDocument(source: Uri, target: File): Long {
        target.parentFile?.mkdirs()
        resolver.openInputStream(source)?.use { input ->
            FileOutputStream(target, false).use { output ->
                return DocumentIo.copyBounded(input, output)
            }
        }
        throw IOException("El proveedor no abrió un flujo de lectura.")
    }

    /** El proveedor puede no permitir borrar; el fallo no oculta el error original. */
    private fun deleteDocumentQuietly(uri: Uri) {
        try {
            DocumentsContract.deleteDocument(resolver, uri)
        } catch (_: Exception) {
            // Sin acción: el error de escritura ya se informa al usuario.
        }
    }

    private fun respond(block: () -> Unit) {
        try {
            block()
        } catch (_: Exception) {
            // La respuesta puede perderse si el proceso ya no está escuchando.
        }
    }

    private fun result(status: String, bytes: Long?): JSObject {
        return JSObject().apply {
            put("status", status)
            if (bytes != null) put("bytes", bytes)
        }
    }

    private companion object {
        /** Executor compartido y daemon: no retiene la Activity ni acumula hilos. */
        val executor: ExecutorService by lazy {
            Executors.newSingleThreadExecutor { runnable ->
                Thread(runnable, "hiqnet-saf-document").apply { isDaemon = true }
            }
        }
    }
}
