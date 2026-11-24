# language: fr
@querying
Fonctionnalité: Lecture seule MongoDB
  L'agent interroge MongoDB par find et aggregate ; seuls les étapes et opérateurs de lecture d'une liste autorisée passent, tout le reste est refusé avec la règle qui l'arrête, avant qu'un pseudonyme soit cherché ou qu'une commande parte.

  Contexte:
    Étant donné l'agent connecté à une base MongoDB

  Scénario: Une agrégation de lecture passe et reçoit la borne de documents
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$match": {"status": "PAID"}}, {"$group": {"_id": "$status", "n": {"$sum": 1}}}]
      """
    Alors la demande est acceptée
    Et le pipeline envoyé se termine par une limite de 51 documents

  Scénario: AE2, un $out en deuxième étape est refusé en nommant l'étape
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$match": {}}, {"$out": "copy"}]
      """
    Alors la demande est refusée par la règle "mongo_stage_not_allowed"
    Et le refus nomme "$out"
    Et le refus nomme "stage 2"
    Et rien n'a été demandé au magasin de pseudonymes

  Scénario: Un $merge caché dans une branche de $facet est refusé
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$facet": {"copie": [{"$match": {}}, {"$merge": {"into": "copy"}}]}}]
      """
    Alors la demande est refusée par la règle "mongo_stage_not_allowed"
    Et le refus nomme "$merge"

  Plan du Scénario: AE3, le JavaScript côté serveur est refusé en nommant l'opérateur
    Quand l'agent cherche avec le filtre suivant :
      """
      <filtre>
      """
    Alors la demande est refusée par la règle "<règle>"
    Et le refus nomme "<nom>"
    Et rien n'a été demandé au magasin de pseudonymes

    Exemples:
      | filtre                                                                    | règle                           | nom       |
      | {"$where": "this.a == 1"}                                                 | mongo_operator_not_allowed      | $where    |
      | {"$expr": {"$function": {"body": "function() {}", "args": [], "lang": "js"}}} | mongo_operator_not_allowed      | $function |
      | {"a": {"$code": "function() {}"}}                                         | mongo_extended_json_not_allowed | $code     |
      | {"status": {"$fooBar": 1}}                                                | mongo_operator_not_allowed      | $fooBar   |

  Scénario: AE4, un $lookup vers une autre base est refusé
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$lookup": {"from": {"db": "admin", "coll": "system.users"}, "localField": "a", "foreignField": "b", "as": "c"}}]
      """
    Alors la demande est refusée par la règle "mongo_cross_database"
    Et le refus nomme "from"

  Scénario: Une collection système est refusée
    Quand l'agent lance l'agrégation suivante :
      """
      [{"$unionWith": "system.users"}]
      """
    Alors la demande est refusée par la règle "mongo_system_namespace"

  Scénario: Un refus ne reprend aucune valeur de la demande
    Quand l'agent cherche avec le filtre suivant :
      """
      {"email": "ylwenna.kervadec@canary-mail.fr", "$where": "this.email == 'ylwenna.kervadec@canary-mail.fr'"}
      """
    Alors la demande est refusée par la règle "mongo_operator_not_allowed"
    Et le refus ne contient aucune valeur canari
