use cucumber::given;

use crate::anonymization_world::{AnonymizationWorld, listed};

#[given(regex = r#"^la base déclare les libellés d'énumération "([^"]+)"$"#)]
async fn database_declares_enum_labels(world: &mut AnonymizationWorld, labels: String) {
    world.enum_labels = listed(&labels);
}
