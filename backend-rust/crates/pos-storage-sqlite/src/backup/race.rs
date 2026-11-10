//! Gancho exclusivo de pruebas para inspeccionar el intervalo entre el
//! checkpoint y la promoción. No existe en compilaciones normales.
use std::sync::Mutex;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Point {
    /// Con el bloqueo exclusivo todavía tomado.
    BeforePromotion,
    /// Después de soltar la guardia y apartar sidecars, justo antes de
    /// reemplazar el archivo: la ventana mínima del procedimiento.
    BeforePromotionUnlocked,
}

type Hook = Box<dyn FnOnce() + Send>;

static HOOK: Mutex<Option<(Point, Hook)>> = Mutex::new(None);

pub fn arm(point: Point, hook: Hook) {
    *HOOK.lock().unwrap_or_else(|poison| poison.into_inner()) = Some((point, hook));
}

pub fn hit(point: Point) {
    let hook = {
        let mut slot = HOOK.lock().unwrap_or_else(|poison| poison.into_inner());
        match slot.as_ref() {
            Some((armed, _)) if *armed == point => slot.take().map(|(_, hook)| hook),
            _ => None,
        }
    };
    if let Some(hook) = hook {
        hook();
    }
}
