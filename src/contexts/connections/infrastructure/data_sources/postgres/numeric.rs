const SIGN_POSITIVE: u16 = 0x0000;
const SIGN_NEGATIVE: u16 = 0x4000;
const SIGN_NAN: u16 = 0xC000;
const SIGN_POSITIVE_INFINITY: u16 = 0xD000;
const SIGN_NEGATIVE_INFINITY: u16 = 0xF000;
const DIGITS_PER_GROUP: usize = 4;

pub(crate) fn numeric_to_string(raw: &[u8]) -> Result<String, String> {
    let header = |index: usize| -> Result<[u8; 2], String> {
        raw.get(index * 2..index * 2 + 2)
            .map(|bytes| [bytes[0], bytes[1]])
            .ok_or_else(|| "truncated numeric header".to_string())
    };
    let ndigits = i16::from_be_bytes(header(0)?);
    let weight = i16::from_be_bytes(header(1)?);
    let sign = u16::from_be_bytes(header(2)?);
    let dscale = u16::from_be_bytes(header(3)?);
    let ndigits = usize::try_from(ndigits).map_err(|_| "negative numeric digit count")?;
    let body = &raw[8..];
    if body.len() != ndigits * 2 {
        return Err("numeric digit count does not match its payload".into());
    }
    let digits = body
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| i16::from_be_bytes(*pair))
        .map(|digit| {
            u16::try_from(digit)
                .ok()
                .filter(|digit| *digit < 10_000)
                .ok_or_else(|| "numeric digit out of range".to_string())
        })
        .collect::<Result<Vec<u16>, String>>()?;
    let negative = match sign {
        SIGN_POSITIVE => false,
        SIGN_NEGATIVE => true,
        SIGN_NAN => return Ok("NaN".into()),
        SIGN_POSITIVE_INFINITY => return Ok("Infinity".into()),
        SIGN_NEGATIVE_INFINITY => return Ok("-Infinity".into()),
        _ => return Err("unknown numeric sign".into()),
    };
    let digit_at = |position: i32| -> u16 {
        usize::try_from(position)
            .ok()
            .and_then(|index| digits.get(index).copied())
            .unwrap_or(0)
    };
    let weight = i32::from(weight);
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if weight < 0 {
        out.push('0');
    } else {
        out.push_str(&digit_at(0).to_string());
        for position in 1..=weight {
            out.push_str(&format!("{:04}", digit_at(position)));
        }
    }
    let scale = usize::from(dscale);
    if scale > 0 {
        let mut fraction = String::with_capacity(scale + DIGITS_PER_GROUP);
        let mut position = weight + 1;
        while fraction.len() < scale {
            fraction.push_str(&format!("{:04}", digit_at(position)));
            position += 1;
        }
        fraction.truncate(scale);
        out.push('.');
        out.push_str(&fraction);
    }
    Ok(out)
}

#[cfg(test)]
mod unit {
    use super::*;

    const POSITIVE: u16 = 0x0000;
    const NEGATIVE: u16 = 0x4000;

    fn encode(weight: i16, sign: u16, dscale: u16, digits: &[i16]) -> Vec<u8> {
        let mut buf = Vec::new();
        buf.extend_from_slice(&(digits.len() as i16).to_be_bytes());
        buf.extend_from_slice(&weight.to_be_bytes());
        buf.extend_from_slice(&sign.to_be_bytes());
        buf.extend_from_slice(&dscale.to_be_bytes());
        for digit in digits {
            buf.extend_from_slice(&digit.to_be_bytes());
        }
        buf
    }

    fn render(weight: i16, sign: u16, dscale: u16, digits: &[i16]) -> String {
        numeric_to_string(&encode(weight, sign, dscale, digits)).unwrap()
    }

    #[test]
    fn l_echelle_declaree_garde_ses_zeros_finaux() {
        assert_eq!(render(1, POSITIVE, 4, &[1, 2345, 6700]), "12345.6700");
    }

    #[test]
    fn zero_s_affiche_avec_et_sans_echelle() {
        assert_eq!(render(0, POSITIVE, 0, &[]), "0");
        assert_eq!(render(0, POSITIVE, 4, &[]), "0.0000");
    }

    #[test]
    fn les_fractions_negatives_inferieures_a_un_sont_rendues() {
        assert_eq!(render(-1, NEGATIVE, 1, &[5000]), "-0.5");
    }

    #[test]
    fn les_groupes_fractionnaires_de_tete_sont_completes_par_des_zeros() {
        assert_eq!(render(-2, POSITIVE, 6, &[1200]), "0.000012");
        assert_eq!(render(-2, POSITIVE, 8, &[12]), "0.00000012");
        assert_eq!(render(-1, POSITIVE, 5, &[1, 2000]), "0.00012");
    }

    #[test]
    fn les_groupes_entiers_omis_sont_restitues_en_zeros_finaux() {
        assert_eq!(render(2, POSITIVE, 0, &[1]), "100000000");
        assert_eq!(render(1, NEGATIVE, 2, &[7, 1]), "-70001.00");
    }

    #[test]
    fn les_decimales_au_dela_de_l_echelle_sont_tronquees() {
        assert_eq!(render(0, POSITIVE, 2, &[3, 1400]), "3.14");
    }

    #[test]
    fn les_valeurs_speciales_sont_rendues() {
        assert_eq!(render(0, 0xC000, 0, &[]), "NaN");
        assert_eq!(render(0, 0xD000, 0, &[]), "Infinity");
        assert_eq!(render(0, 0xF000, 0, &[]), "-Infinity");
    }

    #[test]
    fn une_entree_tronquee_est_rejetee() {
        let mut buf = encode(1, POSITIVE, 4, &[1, 2345, 6700]);
        buf.truncate(buf.len() - 1);
        assert!(numeric_to_string(&buf).is_err());
        assert!(numeric_to_string(&[0, 1]).is_err());
    }

    #[test]
    fn les_chiffres_hors_plage_et_signes_inconnus_sont_rejetes() {
        assert!(numeric_to_string(&encode(0, POSITIVE, 0, &[10000])).is_err());
        assert!(numeric_to_string(&encode(0, 0x1234, 0, &[1])).is_err());
    }
}
