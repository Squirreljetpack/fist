use cli_boilerplate_automation::bring::consume_escaped;
use serde::Deserialize;

pub fn escaped_opt_char<'de, D>(deserializer: D) -> Result<Option<char>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let opt = Option::<String>::deserialize(deserializer)?;
    match opt {
        Some(s) => {
            let mut out = String::with_capacity(s.len());
            let mut chars = s.chars();
            while let Some(c) = chars.next() {
                if c == '\\' {
                    consume_escaped(&mut chars, &mut out);
                    continue;
                }
                out.push(c)
            }

            let mut chars = out.chars();
            let first = chars
                .next()
                .ok_or_else(|| serde::de::Error::custom("escaped string is empty"))?;
            if chars.next().is_some() {
                return Err(serde::de::Error::custom(
                    "escaped string must be exactly one character",
                ));
            }
            Ok(Some(first))
        }
        None => Ok(None),
    }
}

pub mod border_result {
    use matchmaker::config::{BorderSetting, PartialBorderSetting};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(
        value: &Result<BorderSetting, PartialBorderSetting>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Ok(full) => full.serialize(serializer),
            Err(partial) => partial.serialize(serializer),
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D
    ) -> Result<Result<BorderSetting, PartialBorderSetting>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let partial = PartialBorderSetting::deserialize(deserializer)?;
        Ok(Err(partial))
    }
}
