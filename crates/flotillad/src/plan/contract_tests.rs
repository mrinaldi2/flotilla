use std::path::PathBuf;

use bus::contract::board as c;

use super::m;

/// The golden plan node survives contract → model → contract unchanged.
#[test]
fn the_plan_node_fixture_survives_the_mapping() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bus/fixtures/board/plan_node.json");
    let wire: c::PlanNode =
        serde_json::from_str(&std::fs::read_to_string(path).expect("fixture")).expect("decodes");
    let model = m::Node::try_from(wire.clone()).expect("maps");
    assert_eq!(model.stage, m::Stage::Hld);
    assert_eq!(model.gate.as_ref().map(|g| g.round), Some(2));
    assert_eq!(c::PlanNode::from(model), wire);
}

/// Every stored spelling has the contract value of the same name.
#[test]
fn stored_spellings_match_the_proto_names() {
    for stage in m::Stage::ALL {
        let wire = c::PlanStage::from(*stage).as_str_name();
        assert_eq!(
            wire,
            format!("PLAN_STAGE_{}", stage.as_str().to_uppercase())
        );
        assert_eq!(m::Stage::from_wire(i32::from(*stage)), Ok(*stage));
    }
    for kind in m::Kind::ALL {
        let wire = c::PlanKind::from(*kind).as_str_name();
        assert_eq!(wire, format!("PLAN_KIND_{}", kind.as_str().to_uppercase()));
    }
    for kind in m::DocKind::ALL {
        let wire = c::PlanDocKind::from(*kind).as_str_name();
        assert_eq!(
            wire,
            format!("PLAN_DOC_KIND_{}", kind.as_str().to_uppercase())
        );
    }
    assert!(m::Stage::from_wire(0).is_err());
    assert!(m::Stage::from_wire(99).is_err());
}
