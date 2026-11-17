//! Icône de bouclier construite en mémoire (pixels calculés puis `CreateIconIndirect`), sans fichier.

use std::ffi::c_void;

use windows_sys::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, DestroyIcon, ICONINFO};

use crate::error::Win32Error;
use crate::ui_text::last_error;

/// Côté de l'icône, en pixels.
pub const ICON_SIZE: u32 = 32;

const PURPLE: u32 = 0xFF70_41C7;
const LIGHT: u32 = 0xFFF9_F6FF;

/// Indique si un point normalisé est dans le bouclier.
///
/// Paramètres : abscisse de -1 à 1 et ordonnée de 0 (haut) à 1 (bas).
/// Retour : vrai pour l'intérieur de la forme.
fn inside_shield(u: f64, v: f64) -> bool {
    if !(0.06..=0.96).contains(&v) {
        return false;
    }
    let half = if v < 0.45 {
        0.82
    } else {
        let t = (v - 0.45) / 0.51;
        0.82 * (1.0 - t * t).max(0.0).sqrt()
    };
    u.abs() <= half
}

/// Indique si un point normalisé est dans la serrure centrale.
///
/// Paramètres : abscisse de -1 à 1 et ordonnée de 0 à 1.
/// Retour : vrai pour le disque ou la tige de la serrure.
fn inside_keyhole(u: f64, v: f64) -> bool {
    let disc = u * u + (v - 0.42) * (v - 0.42) <= 0.02;
    let stem = u.abs() <= 0.07 && (0.42..=0.68).contains(&v);
    disc || stem
}

/// Calcule les pixels BGRA (`0xAARRGGBB`) du bouclier.
///
/// Paramètres : le côté en pixels.
/// Retour : `size * size` pixels, ligne du haut en premier ; fond transparent, bouclier violet, serrure claire.
#[must_use]
pub fn shield_pixels(size: u32) -> Vec<u32> {
    let scale = f64::from(size);
    (0..size * size)
        .map(|index| {
            let u = (f64::from(index % size) + 0.5) / scale * 2.0 - 1.0;
            let v = (f64::from(index / size) + 0.5) / scale;
            match (inside_shield(u, v), inside_keyhole(u, v)) {
                (true, true) => LIGHT,
                (true, false) => PURPLE,
                _ => 0,
            }
        })
        .collect()
}

/// Icône système possédée, détruite à la fin de sa vie.
#[derive(Debug)]
pub struct Icon(isize);

impl Icon {
    /// Construit l'icône de bouclier de `ICON_SIZE` pixels.
    ///
    /// Retour : l'icône, ou l'erreur `Api` de la création des bitmaps ou de l'icône.
    ///
    /// # Errors
    ///
    /// Les erreurs possibles sont celles décrites dans Retour.
    ///
    /// Sûreté : les pointeurs de pixels et de masque visent des tampons locaux de la taille exacte annoncée ; GDI en
    /// copie le contenu pendant `CreateBitmap`, et les bitmaps sont libérés après `CreateIconIndirect`.
    pub fn shield() -> Result<Self, Win32Error> {
        let pixels = shield_pixels(ICON_SIZE);
        let mask = vec![0_u8; usize::try_from(ICON_SIZE * ICON_SIZE.div_ceil(16) * 2).unwrap_or(0)];
        let side = i32::try_from(ICON_SIZE).unwrap_or(32);
        unsafe {
            let color = CreateBitmap(side, side, 1, 32, pixels.as_ptr().cast::<c_void>());
            let mask = CreateBitmap(side, side, 1, 1, mask.as_ptr().cast::<c_void>());
            if color.is_null() || mask.is_null() {
                let error = last_error("CreateBitmap");
                DeleteObject(color.cast());
                DeleteObject(mask.cast());
                return Err(error);
            }
            let info = ICONINFO {
                fIcon: 1,
                xHotspot: 0,
                yHotspot: 0,
                hbmMask: mask,
                hbmColor: color,
            };
            let icon = CreateIconIndirect(&raw const info);
            DeleteObject(color.cast());
            DeleteObject(mask.cast());
            if icon.is_null() {
                return Err(last_error("CreateIconIndirect"));
            }
            Ok(Self(icon as isize))
        }
    }

    /// Identifiant système de l'icône.
    ///
    /// Retour : le `HICON` en entier, valable tant que l'icône vit.
    #[must_use]
    pub fn raw(&self) -> isize {
        self.0
    }
}

impl Drop for Icon {
    /// Détruit l'icône système.
    ///
    /// Sûreté : l'identifiant vient de `CreateIconIndirect` et n'est détruit qu'ici.
    fn drop(&mut self) {
        unsafe {
            DestroyIcon(self.0 as *mut c_void);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_have_the_right_size_and_shape() {
        let pixels = shield_pixels(ICON_SIZE);
        let side = usize::try_from(ICON_SIZE).unwrap_or(0);
        assert_eq!(pixels.len(), side * side);
        assert_eq!(pixels[0], 0);
        assert_eq!(pixels[side * 8 + 6], PURPLE);
        assert_eq!(pixels[side * 13 + 16], LIGHT);
        assert_eq!(pixels[side * 31 + 16], 0);
    }
}
