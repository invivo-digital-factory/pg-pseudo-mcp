use cucumber::{then, when};
use serde_json::json;

use crate::querying_world::QueryingWorld;

#[then(regex = r"^la première cellule de la réponse est un pseudonyme$")]
async fn first_cell_is_a_pseudonym(world: &mut QueryingWorld) {
    let answer = world.answer();
    assert!(!answer.refused, "{}", answer.text);
    let pseudonym = answer.rows()[0][0].as_str().expect("text cell").to_string();
    assert_eq!(answer.body()["pseudonymized"]["email"]["kind"], "email");
    assert!(!pseudonym.contains("canary-mail"), "{pseudonym}");
    world.pseudonym = Some(pseudonym);
}

#[when(regex = r"^l'agent cherche les commandes du client dont l'adresse est ce pseudonyme$")]
async fn agent_looks_up_orders(world: &mut QueryingWorld) {
    let pseudonym = world.pseudonym.clone().expect("a pseudonym was read");
    let sql = format!(
        "SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = '{pseudonym}' ORDER BY o.id"
    );
    world.ask(&sql).await;
}

#[then(regex = r"^la réponse liste les commandes 10 et 11$")]
async fn answer_lists_orders(world: &mut QueryingWorld) {
    assert_eq!(world.answer().body()["rows"], json!([[10], [11]]));
}

#[then(regex = r"^le pseudonyme a été traduit en une valeur réelle côté serveur$")]
async fn pseudonym_was_translated(world: &mut QueryingWorld) {
    assert_eq!(world.answer().body()["translated_literals"], 1);
}
