// Caller-only observations. Text is supplied by the host/emitted runtime;
// all value and identity operations delegate to production components.
fn units(value: &Text) -> String {
    value
        .units()
        .iter()
        .map(|x| format!("{x:x}"))
        .collect::<Vec<_>>()
        .join(",")
}
fn push(output: &mut Vec<String>, key: &str, value: impl std::fmt::Display) {
    output.push(format!("{key}={value}"));
}
pub fn observations() -> Vec<String> {
    let mut output = Vec::new();
    let arrays: Vec<Vec<u16>> = vec![
        vec![],
        vec![65],
        vec![0],
        vec![65, 233, 55357, 56832, 90],
        vec![55357],
        vec![56832],
        vec![55357, 56832],
        vec![56832, 55357],
        vec![55296, 55296, 56320],
        vec![65535, 0, 1],
        vec![97, 98, 97, 98, 97],
        vec![55357, 65, 56832],
        vec![10],
        vec![233, 233, 233],
    ];
    let strings: Vec<_> = arrays.into_iter().map(Text::from_units).collect();
    let offsets = [-2, -1, 0, 1, 2, 5, i32::MAX];
    let points = [
        -1, 0, 65, 233, 55357, 56832, 65535, 65536, 128512, 1114111, 1114112,
    ];
    let empty = Text::from_units(vec![]).intern();
    for (i, s) in strings.iter().enumerate() {
        let k = format!("s.{i}");
        push(&mut output, &format!("{k}.units"), units(s));
        push(&mut output, &format!("{k}.length"), s.len());
        push(&mut output, &format!("{k}.hash"), s.java_hash());
        push(
            &mut output,
            &format!("{k}.cloneIdentity"),
            s.java_identity_eq(&s.clone()),
        );
        let fresh = Text::from_units(s.units().to_vec());
        push(
            &mut output,
            &format!("{k}.freshIdentity"),
            s.java_identity_eq(&fresh),
        );
        push(
            &mut output,
            &format!("{k}.internIdentity"),
            s.intern().java_identity_eq(&fresh.intern()),
        );
        push(
            &mut output,
            &format!("{k}.wholeSliceIdentity"),
            s.substring(0, s.len()).unwrap().java_identity_eq(s),
        );
        push(&mut output, &format!("{k}.trimUnits"), units(&s.trim()));
        push(
            &mut output,
            &format!("{k}.trimIdentity"),
            s.trim().java_identity_eq(s),
        );
        push(
            &mut output,
            &format!("{k}.emptyConcatIdentity"),
            s.concat(&empty).java_identity_eq(s),
        );
        push(
            &mut output,
            &format!("{k}.emptyAppendIdentity"),
            s.append(&empty).java_identity_eq(s),
        );
        push(
            &mut output,
            &format!("{k}.emptySliceIdentity"),
            s.substring(s.len(), s.len())
                .unwrap()
                .java_identity_eq(&empty),
        );
        for n in -1..=s.len() {
            push(
                &mut output,
                &format!("{k}.char.{n}"),
                s.char_at(n)
                    .map(|u| format!("{:x}", u.as_u16()))
                    .unwrap_or_else(|_| "ERR".into()),
            );
        }
        for b in -1..=s.len() + 1 {
            for e in -1..=s.len() + 1 {
                push(
                    &mut output,
                    &format!("{k}.sub.{b}.{e}"),
                    s.substring(b, e)
                        .map(|v| units(&v))
                        .unwrap_or_else(|_| "ERR".into()),
                );
            }
        }
        let mut buffer = s.payload().clone();
        buffer.push_text(Text::from_units(vec![0xd800]).payload());
        push(
            &mut output,
            &format!("{k}.buffer.append"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer
            .set_unit(0, Text::from_units(vec![0xdc00]).char_at(0).unwrap())
            .unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.set"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer.set_length(1).unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.truncate"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer.set_length(3).unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.grow"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer
            .replace_range(1, 1, Text::from_units(vec![0xd83d, 0xde00]).payload())
            .unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.insert"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer
            .replace_range(1, 2, Text::from_scalar("").payload())
            .unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.delete"),
            units(&Text::from_payload(buffer.clone())),
        );
        buffer
            .replace_range(0, 2, Text::from_scalar("é").payload())
            .unwrap();
        push(
            &mut output,
            &format!("{k}.buffer.replace"),
            units(&Text::from_payload(buffer.clone())),
        );
        for (j, t) in strings.iter().enumerate() {
            let p = format!("{k}.other.{j}");
            push(&mut output, &format!("{p}.eq"), s == t);
            push(&mut output, &format!("{p}.cmp"), s.java_compare(t));
            push(&mut output, &format!("{p}.concat"), units(&s.concat(t)));
            push(
                &mut output,
                &format!("{p}.concatSelfIdentity"),
                s.concat(t).java_identity_eq(s),
            );
            push(
                &mut output,
                &format!("{p}.concatOtherIdentity"),
                s.concat(t).java_identity_eq(t),
            );
            for offset in offsets {
                push(
                    &mut output,
                    &format!("{p}.index.{offset}"),
                    s.index_of(t, offset),
                );
                push(
                    &mut output,
                    &format!("{p}.last.{offset}"),
                    s.last_index_of(t, offset),
                );
            }
        }
        for point in points {
            for offset in [-1, 0, 1, 5] {
                push(
                    &mut output,
                    &format!("{k}.point.{point}.{offset}"),
                    s.index_of_code_point(point, offset),
                );
            }
        }
    }
    let unique = Text::from_units(vec![0xd832, 0x0001, 0xffff, 0xdc22, 0x0327]);
    push(
        &mut output,
        "uniqueFirstInternIdentity",
        unique.intern().java_identity_eq(&unique),
    );
    output
}
