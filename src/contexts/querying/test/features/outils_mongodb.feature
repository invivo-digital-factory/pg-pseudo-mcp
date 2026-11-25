# language: fr
@querying
Fonctionnalité: Outils MongoDB de bout en bout
  Une connexion MongoDB se lit avec find et aggregate ; query et explain la renvoient vers ces outils.
  Chaque valeur personnelle revient pseudonymisée, y compris dans les erreurs du serveur, et un pseudonyme comparé à un champ est retraduit.

  @integration
  Scénario: query sur une connexion MongoDB renvoie vers find ou aggregate
    Étant donné la connexion MongoDB de test à travers le bastion
    Quand l'agent exécute la requête SQL "SELECT 1" sur la connexion MongoDB
    Alors l'outil est refusé et renvoie vers "find"

  @integration
  Scénario: Un client suivi d'une collection à l'autre par le pseudonyme de son email
    Étant donné la connexion MongoDB de test à travers le bastion
    Quand l'agent cherche dans "customers" le document d'identifiant "65a000000000000000000001"
    Alors le champ "contact.email" est pseudonymisé comme "email"
    Quand l'agent cherche dans "orders" les documents dont "customer.email" vaut ce pseudonyme
    Alors la réponse compte 2 documents
    Et un pseudonyme a été retraduit

  @integration
  Scénario: L'erreur de conversion du serveur ne cite jamais le nom réel
    Étant donné la connexion MongoDB de test à travers le bastion
    Quand l'agent agrège "customers" avec [{"$project": {"n": {"$toInt": "$lastName"}}}]
    Alors l'outil est refusé avec le code "mongodb_error"
    Et la réponse ne contient pas "Kervadec"
