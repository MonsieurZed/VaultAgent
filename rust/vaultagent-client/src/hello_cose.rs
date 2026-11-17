//! Conversion d'un justificatif Windows Hello créé en `HelloCredential` : contrôle des données de l'authentificateur
//! puis lecture de la clé publique COSE (EC2 P-256 ou RSA), comme `HelloVerification.ReadRegistration` du client .NET.

use vaultagent_core::encoding::{base64_encode, hex_encode};
use vaultagent_core::{HelloCredential, digest, hello_public_key};

use crate::error::ClientError;
use crate::hello_system::HelloRegistration;

const FLAGS_OFFSET: usize = 32;
const ID_LENGTH_OFFSET: usize = 53;
const ID_OFFSET: usize = 55;
const FLAG_USER_PRESENT_AND_VERIFIED: u8 = 5;
const FLAG_ATTESTED_DATA: u8 = 64;
const MAX_ENTRIES: u64 = 16;
const MAX_BYTES: u64 = 1024;
const COSE_KEY_TYPE: i64 = 1;
const COSE_ALGORITHM: i64 = 3;
const COSE_CURVE: i64 = -1;
const COSE_X: i64 = -2;
const COSE_Y: i64 = -3;
const KTY_EC2: i64 = 2;
const KTY_RSA: i64 = 3;
const ALG_ES256: i64 = -7;
const ALG_RS256: i64 = -257;
const CURVE_P256: i64 = 1;

/// Valeur d'une entrée de la clé COSE : entier ou chaîne d'octets (les seules formes d'une clé EC2 ou RSA).
enum Value {
    Number(i64),
    Bytes(Vec<u8>),
}

/// Lecteur CBOR minimal, limité aux entiers, chaînes d'octets et tableaux associatifs d'une clé COSE.
struct Reader<'a> {
    data: &'a [u8],
}

impl Reader<'_> {
    /// Prend `count` octets au début des données restantes.
    fn take(&mut self, count: usize) -> Option<&[u8]> {
        let (head, tail) = self.data.split_at_checked(count)?;
        self.data = tail;
        Some(head)
    }

    /// Lit un octet de tête CBOR et son argument.
    ///
    /// Retour : le type majeur et l'argument (valeur, longueur ou nombre d'entrées).
    fn head(&mut self) -> Option<(u8, u64)> {
        let first = *self.take(1)?.first()?;
        let argument = match first & 31 {
            small @ 0..=23 => u64::from(small),
            24 => u64::from(*self.take(1)?.first()?),
            25 => u64::from(u16::from_be_bytes(self.take(2)?.try_into().ok()?)),
            26 => u64::from(u32::from_be_bytes(self.take(4)?.try_into().ok()?)),
            _ => return None,
        };
        Some((first >> 5, argument))
    }

    /// Lit un entier ou une chaîne d'octets de moins de 1 Kio.
    fn value(&mut self) -> Option<Value> {
        let (major, argument) = self.head()?;
        match major {
            0 => i64::try_from(argument).ok().map(Value::Number),
            1 => i64::try_from(argument).ok().map(|n| Value::Number(-1 - n)),
            2 if argument <= MAX_BYTES => Some(Value::Bytes(self.take(usize::try_from(argument).ok()?)?.to_vec())),
            _ => None,
        }
    }

    /// Lit un tableau associatif de 16 entrées au plus, aux clés entières.
    fn map(&mut self) -> Option<Vec<(i64, Value)>> {
        let (major, count) = self.head()?;
        if major != 5 || count > MAX_ENTRIES {
            return None;
        }
        (0..count)
            .map(|_| match self.value()? {
                Value::Number(key) => Some((key, self.value()?)),
                Value::Bytes(_) => None,
            })
            .collect()
    }
}

/// Erreur d'une inscription refusée.
fn invalid(reason: &str) -> ClientError {
    ClientError::Hello(format!("Windows Hello registration refused: {reason}."))
}

/// Contrôle les données de l'authentificateur et rend la clé COSE qui suit l'identifiant.
fn cose_part<'a>(registration: &'a HelloRegistration, rp_id: &str) -> Result<&'a [u8], ClientError> {
    let auth = registration.authenticator_data.as_slice();
    let rp_hash = hex_encode(
        auth.get(..FLAGS_OFFSET)
            .ok_or_else(|| invalid("authenticator data too short"))?,
    );
    let flags = auth.get(FLAGS_OFFSET).copied().unwrap_or(0);
    let accepted = rp_hash == digest(rp_id.as_bytes())
        && flags & FLAG_USER_PRESENT_AND_VERIFIED == FLAG_USER_PRESENT_AND_VERIFIED
        && flags & FLAG_ATTESTED_DATA != 0;
    if !accepted {
        return Err(invalid("wrong site or missing user verification"));
    }
    let length = auth
        .get(ID_LENGTH_OFFSET..ID_OFFSET)
        .and_then(|bytes| <[u8; 2]>::try_from(bytes).ok())
        .map(u16::from_be_bytes)
        .ok_or_else(|| invalid("attested data incomplete"))?;
    let id_end = ID_OFFSET + usize::from(length);
    let id = auth
        .get(ID_OFFSET..id_end)
        .ok_or_else(|| invalid("attested data incomplete"))?;
    if id != registration.credential_id.as_slice() {
        return Err(invalid("credential id mismatch"));
    }
    auth.get(id_end..).ok_or_else(|| invalid("attested data incomplete"))
}

/// Retrouve une entrée de la clé COSE.
fn entry(entries: &[(i64, Value)], key: i64) -> Option<&Value> {
    entries
        .iter()
        .find(|(candidate, _)| *candidate == key)
        .map(|(_, value)| value)
}

/// Retrouve une entrée entière de la clé COSE.
fn number(entries: &[(i64, Value)], key: i64) -> Option<i64> {
    match entry(entries, key)? {
        Value::Number(value) => Some(*value),
        Value::Bytes(_) => None,
    }
}

/// Retrouve une entrée en octets de la clé COSE, encodée en base64 standard.
fn bytes(entries: &[(i64, Value)], key: i64) -> Option<String> {
    match entry(entries, key)? {
        Value::Bytes(value) => Some(base64_encode(value)),
        Value::Number(_) => None,
    }
}

/// Convertit un justificatif créé par Windows en `HelloCredential`.
///
/// Paramètres : l'identifiant et les données de l'authentificateur rendus par le système, et le RP attendu.
/// Retour : le justificatif (`EC2` P-256 : x et y ; `RSA` : module et exposant), contrôlé par `hello_public_key` de
/// `vaultagent-core`.
///
/// # Errors
///
/// `Hello` si le RP, les drapeaux, l'identifiant ou la clé COSE sont incorrects, ou si l'algorithme n'est pas pris
/// en charge.
pub fn credential_from_registration(
    registration: &HelloRegistration,
    rp_id: &str,
) -> Result<HelloCredential, ClientError> {
    let mut reader = Reader {
        data: cose_part(registration, rp_id)?,
    };
    let entries = reader.map().ok_or_else(|| invalid("unreadable public key"))?;
    let (key_type, parts) = match (number(&entries, COSE_KEY_TYPE), number(&entries, COSE_ALGORITHM)) {
        (Some(KTY_EC2), Some(ALG_ES256)) if number(&entries, COSE_CURVE) == Some(CURVE_P256) => {
            ("EC2", (COSE_X, COSE_Y))
        }
        (Some(KTY_RSA), Some(ALG_RS256)) => ("RSA", (COSE_CURVE, COSE_X)),
        _ => return Err(invalid("unsupported algorithm")),
    };
    let credential = HelloCredential {
        id: base64_encode(&registration.credential_id),
        key_type: key_type.to_owned(),
        first: bytes(&entries, parts.0).ok_or_else(|| invalid("incomplete public key"))?,
        second: bytes(&entries, parts.1).ok_or_else(|| invalid("incomplete public key"))?,
    };
    hello_public_key(Some(&credential)).map_err(|_| invalid("invalid public key"))?;
    Ok(credential)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hello_fixture::{ID, ec2_key, generator, registration, rsa_key};
    use vaultagent_core::HELLO_RP_ID;

    #[test]
    fn ec2_key_becomes_a_credential_core_accepts() {
        let (x, y) = generator();
        let created = registration(HELLO_RP_ID, 0x45, &ec2_key(&x, &y));
        let credential = credential_from_registration(&created, HELLO_RP_ID).unwrap();
        assert_eq!(credential.key_type, "EC2");
        assert_eq!(credential.id, base64_encode(&ID));
        assert_eq!(credential.first, base64_encode(&x));
        assert_eq!(credential.second, base64_encode(&y));
    }

    #[test]
    fn rsa_key_becomes_modulus_and_exponent() {
        let modulus: Vec<u8> = (0..=255u8).map(|n| n | 1).collect();
        let created = registration(HELLO_RP_ID, 0x45, &rsa_key(&modulus));
        let credential = credential_from_registration(&created, HELLO_RP_ID).unwrap();
        assert_eq!(credential.key_type, "RSA");
        assert_eq!(credential.first, base64_encode(&modulus));
        assert_eq!(credential.second, base64_encode(&[1, 0, 1]));
    }

    #[test]
    fn wrong_site_flags_or_id_are_refused() {
        let (x, y) = generator();
        let key = ec2_key(&x, &y);
        for created in [
            registration("other.local", 0x45, &key),
            registration(HELLO_RP_ID, 0x41, &key),
            registration(HELLO_RP_ID, 0x05, &key),
        ] {
            assert_eq!(
                credential_from_registration(&created, HELLO_RP_ID).unwrap_err().code(),
                "hello-failed"
            );
        }
        let mut other_id = registration(HELLO_RP_ID, 0x45, &key);
        other_id.credential_id = vec![8; 16];
        assert!(credential_from_registration(&other_id, HELLO_RP_ID).is_err());
    }

    #[test]
    fn unsupported_or_broken_keys_are_refused() {
        let (x, y) = generator();
        let mut other_curve = ec2_key(&x, &y);
        other_curve[6] = 2;
        let off_curve = ec2_key(&[1; 32], &[2; 32]);
        for cose in [other_curve, off_curve, vec![0xa0], vec![0xff, 0x00], Vec::new()] {
            let created = registration(HELLO_RP_ID, 0x45, &cose);
            assert!(credential_from_registration(&created, HELLO_RP_ID).is_err());
        }
        let short = HelloRegistration {
            credential_id: ID.to_vec(),
            authenticator_data: vec![0; 40],
        };
        assert!(credential_from_registration(&short, HELLO_RP_ID).is_err());
    }
}
