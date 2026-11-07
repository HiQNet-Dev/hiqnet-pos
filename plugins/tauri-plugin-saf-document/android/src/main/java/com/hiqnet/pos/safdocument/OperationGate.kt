package com.hiqnet.pos.safdocument

/**
 * Marca exclusiva para una operación de documentos.
 *
 * La operación permanece activa hasta que la copia termina (éxito o fallo),
 * no cuando el selector devuelve el control. Así dos exportaciones seguidas no
 * se solapan con una copia todavía en curso.
 */
internal class OperationGate {
    private var active = false

    @Synchronized
    fun begin(): Boolean {
        if (active) return false
        active = true
        return true
    }

    @Synchronized
    fun end() {
        active = false
    }

    @Synchronized
    fun isActive(): Boolean = active
}
