//! Lecture d'un paramètre de requête (`?nom=valeur`), comme `URLSearchParams.get` de Node.

/// Valeur décodée du premier paramètre de ce nom.
///
/// Paramètres : la cible de la requête (chemin et requête), le nom du paramètre.
/// Retour : la valeur décodée (`+` vaut espace, `%XX` est décodé en UTF-8), ou `None` si le paramètre manque.
#[must_use]
pub fn query_param(target: &str, name: &str) -> Option<String> {
    let (_, query) = target.split_once('?')?;
    let query = query.split_once('#').map_or(query, |(before, _)| before);
    query
        .split('&')
        .map(|pair| pair.split_once('=').unwrap_or((pair, "")))
        .find(|(key, _)| decode(key) == name)
        .map(|(_, value)| decode(value))
}

/// Décode `+` et les séquences `%XX` ; une séquence invalide reste telle quelle.
pub(crate) fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' => match hex_pair(bytes.get(index + 1..index + 3)) {
                Some(byte) => {
                    out.push(byte);
                    index += 2;
                }
                None => out.push(b'%'),
            },
            other => out.push(other),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Octet désigné par deux chiffres hexadécimaux.
fn hex_pair(digits: Option<&[u8]>) -> Option<u8> {
    let digits = digits?;
    let high = char::from(digits[0]).to_digit(16)?;
    let low = char::from(digits[1]).to_digit(16)?;
    u8::try_from(high * 16 + low).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_decodes_and_ignores_fragment() {
        assert_eq!(
            query_param("/a?x=1&vaultId=ab%2Dc+d#z", "vaultId").as_deref(),
            Some("ab-c d")
        );
        assert_eq!(query_param("/a?vaultId=", "vaultId").as_deref(), Some(""));
        assert_eq!(query_param("/a?x=1", "vaultId"), None);
        assert_eq!(query_param("/a", "vaultId"), None);
        assert_eq!(query_param("/a?v=%zz%4", "v").as_deref(), Some("%zz%4"));
    }
}
