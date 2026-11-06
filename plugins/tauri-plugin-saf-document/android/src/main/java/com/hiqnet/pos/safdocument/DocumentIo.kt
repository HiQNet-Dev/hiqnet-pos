package com.hiqnet.pos.safdocument

import java.io.IOException
import java.io.InputStream
import java.io.OutputStream

/** El documento supera el tamaño admitido para un respaldo de HiQNet POS. */
internal class DocumentTooLargeException(limit: Long) :
    IOException("el documento supera el límite de $limit bytes")

/**
 * Copia en streaming con tope de tamaño.
 *
 * No se guarda el documento en memoria: solo se usa un búfer acotado, de modo
 * que un archivo enorme no agota la memoria del proceso. Al superar el tope se
 * aborta y el llamador elimina el destino parcial.
 */
internal object DocumentIo {
    const val MAX_DOCUMENT_BYTES: Long = 512L * 1024 * 1024
    private const val BUFFER_BYTES = 64 * 1024

    fun copyBounded(
        input: InputStream,
        output: OutputStream,
        limit: Long = MAX_DOCUMENT_BYTES,
    ): Long {
        val buffer = ByteArray(BUFFER_BYTES)
        var total = 0L
        while (true) {
            val read = input.read(buffer)
            if (read == -1) break
            total += read
            if (total > limit) throw DocumentTooLargeException(limit)
            output.write(buffer, 0, read)
        }
        output.flush()
        return total
    }
}
