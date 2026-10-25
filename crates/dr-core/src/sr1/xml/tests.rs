use crate::sr1::xml::*;

#[test]
fn attributes_children_and_case_are_ordered() {
    let element = parse(r#"<Runtime shipId="3" ShipId="4"><ShipNode id="2"/><PlanetNode name="Moon"/><ShipNode id="1"/></Runtime>"#).unwrap();
    assert_eq!(element.attr("shipId"), Some("3"));
    assert_eq!(element.attr("ShipId"), Some("4"));
    assert_eq!(
        element
            .children_named("ShipNode")
            .map(|node| node.attr("id").unwrap())
            .collect::<Vec<_>>(),
        ["2", "1"]
    );
    assert_eq!(element.attributes[0].0, "shipId");
    assert_eq!(parse(&to_string(&element).unwrap()).unwrap(), element);
}

#[test]
fn modifications_preserve_existing_attribute_position() {
    let mut element = Element::new("Ship");
    element
        .set_attr("version", "0")
        .set_attr("liftedOff", "0")
        .set_attr("version", "1");
    assert_eq!(
        element.attributes,
        [
            ("version".into(), "1".into()),
            ("liftedOff".into(), "0".into())
        ]
    );
    assert_eq!(element.remove_attr("version"), Some("1".into()));
    assert_eq!(element.remove_attr("version"), None);
}

#[test]
fn printer_uses_native_five_character_escaping_once() {
    let mut element = Element::new("Pod");
    element.set_attr("name", "A & B <C> \"中\" 'x' &amp;");
    let xml = to_string(&element).unwrap();
    assert_eq!(
        xml,
        r#"<Pod name="A &amp; B &lt;C&gt; &quot;中&quot; &apos;x&apos; &amp;amp;"/>"#
    );
    assert_eq!(parse(&xml).unwrap().attr("name"), element.attr("name"));
}

#[test]
fn printer_does_not_silently_change_control_character_lexemes() {
    let mut element = Element::new("Pod");
    element.set_attr("name", "A\tB\nC\rD");
    assert_eq!(to_string(&element).unwrap(), "<Pod name=\"A\tB\nC\rD\"/>");
}

#[test]
fn text_and_character_references_decode_once() {
    let element = parse("<Name>A &amp; B &#x4e2d;<![CDATA[<C>]]></Name>").unwrap();
    assert_eq!(element.text, "A & B 中<C>");
    assert_eq!(parse(&to_string(&element).unwrap()).unwrap(), element);
}

#[test]
fn prolog_comments_and_utf8_bom_are_not_business_fields() {
    let element = parse("\u{feff}<?xml version='1.0'?><!-- note --><Ship/><!-- done -->").unwrap();
    assert_eq!(element, Element::new("Ship"));
    assert_eq!(to_string(&element).unwrap(), "<Ship/>");
}

#[test]
fn malformed_documents_fail_before_state_processing() {
    for xml in [
        "",
        "text",
        "<Ship>",
        "<A></B>",
        "<A/><B/>",
        "<A/>tail",
        "<A x='1' x='2'/>",
        "<A>&unknown;</A>",
    ] {
        assert!(parse(xml).is_err(), "{xml}");
    }
}

#[test]
fn explicit_root_errors_do_not_convert_data() {
    assert!(Element::new("Ship").require_name("Ship").is_ok());
    assert!(matches!(
        Element::new("Runtime").require_name("Ship"),
        Err(Error::WrongRoot {
            expected: "Ship",
            ..
        })
    ));
}

#[test]
fn invalid_constructed_names_and_values_do_not_produce_bad_xml() {
    for name in ["", "1Ship", "Ship child", "Ship>"] {
        assert!(to_string(&Element::new(name)).is_err());
    }
    let mut element = Element::new("Ship");
    element.set_attr("name", "bad\0value");
    assert!(to_string(&element).is_err());
    element.attributes = vec![("x".into(), "1".into()), ("x".into(), "2".into())];
    assert!(to_string(&element).is_err());
}

#[test]
fn depth_is_bounded_in_reader_and_writer() {
    let input = format!(
        "{}{}",
        "<A>".repeat(MAX_DEPTH + 1),
        "</A>".repeat(MAX_DEPTH + 1)
    );
    assert!(parse(&input).is_err());
    let mut element = Element::new("A");
    for _ in 0..MAX_DEPTH {
        let mut parent = Element::new("A");
        parent.children.push(element);
        element = parent;
    }
    assert!(to_string(&element).is_err());
}
