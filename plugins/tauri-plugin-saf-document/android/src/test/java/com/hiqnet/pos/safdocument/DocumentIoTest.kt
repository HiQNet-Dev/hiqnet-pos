package com.hiqnet.pos.safdocument

import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.IOException
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class DocumentIoTest {
    @Test
    fun copiaTodoElContenidoYDevuelveElTotal() {
        val payload = ByteArray(200_000) { index -> (index % 251).toByte() }
        val output = ByteArrayOutputStream()

        val copied = DocumentIo.copyBounded(ByteArrayInputStream(payload), output)

        assertEquals(payload.size.toLong(), copied)
        assertArrayEquals(payload, output.toByteArray())
    }

    @Test
    fun rechazaUnDocumentoQueSuperaElLimite() {
        val payload = ByteArray(4096)
        val output = ByteArrayOutputStream()

        val error = assertThrows(DocumentTooLargeException::class.java) {
            DocumentIo.copyBounded(ByteArrayInputStream(payload), output, limit = 1024)
        }

        assertEquals("el documento supera el límite de 1024 bytes", error.message)
    }

    @Test
    fun noEscribeMasAllaDelLimiteAceptado() {
        val payload = ByteArray(4096) { 7 }
        val output = ByteArrayOutputStream()

        assertThrows(DocumentTooLargeException::class.java) {
            DocumentIo.copyBounded(ByteArrayInputStream(payload), output, limit = 1024)
        }

        // El búfer es de 64 KiB: el primer bloque ya supera el tope y no debe
        // escribirse nada parcialmente aceptado.
        assertEquals(0, output.size())
    }

    @Test
    fun propagaErroresDeLectura() {
        val broken = object : java.io.InputStream() {
            override fun read(): Int = throw IOException("proveedor interrumpido")
        }

        assertThrows(IOException::class.java) {
            DocumentIo.copyBounded(broken, ByteArrayOutputStream())
        }
    }
}
