package com.hiqnet.pos.safdocument

import android.app.Activity
import android.content.Intent
import android.net.Uri
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
 */
@TauriPlugin
class SafDocumentPlugin(private val activity: Activity) : Plugin(activity) {
    private val executor = Executors.newSingleThreadExecutor()
    private val pendingExports = mutableMapOf<Long, File>()
    private val pendingImports = mutableMapOf<Long, File>()

    @Command
    fun exportDocument(invoke: Invoke) {
        val args = invoke.parseArgs(ExportDocumentArgs::class.java)
        val source = args.sourcePath?.let(::File)
        val name = args.displayName?.trim()
        if (source == null || !source.isFile || name.isNullOrEmpty() || name.contains('/') || name.contains('\\')) {
            invoke.reject("No fue posible preparar el respaldo para exportarlo.", "invalid_request")
            return
        }
        synchronized(pendingExports) {
            if (pendingExports.isNotEmpty() || pendingImports.isNotEmpty()) {
                invoke.reject("Ya hay una operación de documentos en curso.", "busy")
                return
            }
            pendingExports[invoke.id] = source
        }
        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("application/x-sqlite3")
            .putExtra(Intent.EXTRA_TITLE, name)
            .addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        startActivityForResult(invoke, intent, "onExportDocumentResult")
    }

    @ActivityCallback
    fun onExportDocumentResult(invoke: Invoke, result: ActivityResult) {
        val source = synchronized(pendingExports) { pendingExports.remove(invoke.id) }
        if (result.resultCode != Activity.RESULT_OK || result.data?.data == null || source == null) {
            invoke.resolve(result("cancelled", null))
            return
        }
        val destination = result.data!!.data!!
        executor.execute {
            try {
                val copied = copyToDocument(source, destination)
                invoke.resolve(result("completed", copied))
            } catch (error: IOException) {
                invoke.reject("No fue posible escribir el respaldo en el destino seleccionado.", "write_failed", error)
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
        synchronized(pendingExports) {
            if (pendingExports.isNotEmpty() || pendingImports.isNotEmpty()) {
                invoke.reject("Ya hay una operación de documentos en curso.", "busy")
                return
            }
            pendingImports[invoke.id] = target
        }
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT)
            .addCategory(Intent.CATEGORY_OPENABLE)
            .setType("*/*")
            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        startActivityForResult(invoke, intent, "onImportDocumentResult")
    }

    @ActivityCallback
    fun onImportDocumentResult(invoke: Invoke, result: ActivityResult) {
        val target = synchronized(pendingImports) { pendingImports.remove(invoke.id) }
        if (result.resultCode != Activity.RESULT_OK || result.data?.data == null || target == null) {
            invoke.resolve(result("cancelled", null))
            return
        }
        val source = result.data!!.data!!
        executor.execute {
            try {
                val copied = copyFromDocument(source, target)
                invoke.resolve(result("completed", copied))
            } catch (error: IOException) {
                target.delete()
                invoke.reject("No fue posible leer el respaldo seleccionado.", "read_failed", error)
            }
        }
    }

    private fun copyToDocument(source: File, destination: Uri): Long {
        activity.contentResolver.openOutputStream(destination, "w")?.use { output ->
            FileInputStream(source).use { input ->
                return input.copyTo(output)
            }
        }
        throw IOException("El proveedor no abrió un flujo de escritura.")
    }

    private fun copyFromDocument(source: Uri, target: File): Long {
        target.parentFile?.mkdirs()
        activity.contentResolver.openInputStream(source)?.use { input ->
            FileOutputStream(target, false).use { output ->
                return input.copyTo(output)
            }
        }
        throw IOException("El proveedor no abrió un flujo de lectura.")
    }

    private fun result(status: String, bytes: Long?): JSObject {
        return JSObject().apply {
            put("status", status)
            if (bytes != null) put("bytes", bytes)
        }
    }
}
