package com.hiqnet.pos.safdocument

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class OperationGateTest {
    @Test
    fun admiteUnaSolaOperacionActiva() {
        val gate = OperationGate()
        assertTrue(gate.begin())
        assertTrue(gate.isActive())
        assertFalse("una segunda operación no debe solaparse", gate.begin())
        gate.end()
        assertFalse(gate.isActive())
        assertTrue("al terminar la copia se libera el turno", gate.begin())
        gate.end()
    }

    @Test
    fun terminarDosVecesNoRompeElTurno() {
        val gate = OperationGate()
        assertTrue(gate.begin())
        gate.end()
        gate.end()
        assertTrue(gate.begin())
        gate.end()
    }

    @Test
    fun laCopiaMantieneLaOperacionActivaHastaTerminar() {
        // Representa el defecto corregido: el callback del selector no debe
        // liberar el turno antes de que la copia termine.
        val gate = OperationGate()
        assertTrue(gate.begin())
        assertTrue("mientras copia, sigue activa", gate.isActive())
        assertFalse(gate.begin())
        gate.end()
        assertTrue(gate.begin())
        gate.end()
    }
}
