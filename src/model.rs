pub fn is_wand_item(key: &str) -> bool {
    matches!(key, "wooden_axe" | "minecraft:wooden_axe")
}
pub const LIMIT: usize = 4096;
pub type Pos = (i32, i32, i32);
pub fn valid(p: Pos, dimension: &str) -> Result<(), String> {
    let (low, high) = if dimension == "minecraft:overworld" {
        (-64, 319)
    } else {
        (0, 255)
    };
    if p.0.abs_diff(0) > 29_999_980 || p.2.abs_diff(0) > 29_999_980 || !(low..=high).contains(&p.1)
    {
        return Err("Position is outside the world's build limits.".into());
    }
    Ok(())
}
pub fn region(a: Pos, b: Pos, dimension: &str) -> Result<Vec<Pos>, String> {
    valid(a, dimension)?;
    valid(b, dimension)?;
    let size = (a.0.abs_diff(b.0) as u64 + 1)
        * (a.1.abs_diff(b.1) as u64 + 1)
        * (a.2.abs_diff(b.2) as u64 + 1);
    if size > LIMIT as u64 {
        return Err(format!("Selection has {size} blocks; limit is {LIMIT}."));
    }
    let mut out = Vec::with_capacity(size as usize);
    for x in a.0.min(b.0)..=a.0.max(b.0) {
        for z in a.2.min(b.2)..=a.2.max(b.2) {
            for y in a.1.min(b.1)..=a.1.max(b.1) {
                out.push((x, y, z));
            }
        }
    }
    Ok(out)
}
pub fn position(raw: &str, fallback: Pos, dimension: &str) -> Result<Pos, String> {
    let words: Vec<_> = raw
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    let p = if words.is_empty() {
        fallback
    } else if words.len() == 3 {
        let nums: Result<Vec<i32>, _> = words.iter().map(|s| s.parse()).collect();
        let n = nums.map_err(|_| "Use whole-number X,Y,Z coordinates.")?;
        (n[0], n[1], n[2])
    } else {
        return Err("Use X,Y,Z or omit coordinates for your position.".into());
    };
    valid(p, dimension)?;
    Ok(p)
}
pub fn material(raw: &str) -> Result<(String, Vec<(String, String)>), String> {
    let (name, props) = if let Some((a, b)) = raw.split_once('[') {
        (
            a,
            Some(b.strip_suffix(']').ok_or("Missing ] in block state.")?),
        )
    } else {
        (raw, None)
    };
    if name.is_empty()
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"_:".contains(&c))
    {
        return Err("Use a block name such as stone or oak_log[axis=y].".into());
    }
    let name = if name.contains(':') {
        name.into()
    } else {
        format!("minecraft:{name}")
    };
    let mut properties = Vec::new();
    if let Some(props) = props {
        for prop in props.split(',') {
            let (k, v) = prop
                .split_once('=')
                .ok_or("Block properties use name=value.")?;
            if k.is_empty()
                || v.is_empty()
                || !k
                    .bytes()
                    .chain(v.bytes())
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
                || properties.iter().any(|(old, _)| old == k)
            {
                return Err("Invalid or duplicate block property.".into());
            }
            properties.push((k.into(), v.into()));
        }
    }
    Ok((name, properties))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wand_registry_keys() {
        assert!(is_wand_item("wooden_axe"));
        assert!(is_wand_item("minecraft:wooden_axe"));
        assert!(!is_wand_item("minecraft:stone_axe"));
    }
    #[test]
    fn reversed_region_and_limits() {
        assert_eq!(
            region((1, 1, 1), (0, 0, 0), "minecraft:overworld")
                .unwrap()
                .len(),
            8
        );
        assert!(region((0, 0, 0), (4096, 0, 0), "minecraft:overworld").is_err());
        assert!(region((i32::MIN, 0, 0), (i32::MAX, 0, 0), "minecraft:overworld").is_err());
    }
    #[test]
    fn parse_positions() {
        assert_eq!(
            position("-2,64,3", (0, 0, 0), "minecraft:overworld").unwrap(),
            (-2, 64, 3)
        );
        assert!(position("0 320 0", (0, 0, 0), "minecraft:overworld").is_err());
        assert!(position("~ ~ ~", (0, 0, 0), "minecraft:overworld").is_err());
    }
    #[test]
    fn parse_materials() {
        assert_eq!(
            material("oak_log[axis=y]").unwrap(),
            (
                "minecraft:oak_log".into(),
                vec![("axis".into(), "y".into())]
            )
        );
        for bad in [
            "stone[",
            "stone[]",
            "stone[axis=x,axis=y]",
            "stone{foo:1}",
            "stone,air",
        ] {
            assert!(material(bad).is_err(), "{bad}");
        }
    }
}
