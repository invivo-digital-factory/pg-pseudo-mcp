# language: fr
@integration @querying
Fonctionnalité: Colonnes dérivées d'une donnée personnelle
  Une valeur calculée à partir d'une adresse e-mail ne doit jamais en révéler un morceau.

  Scénario: Le début d'une adresse découpé en SQL revient pseudonymisé
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT split_part(email, '@', 1) AS debut FROM customers WHERE id = 1"
    Alors la requête réussit
    Et la colonne "debut" est signalée comme dérivée et pseudonymisée
    Et la réponse ne contient aucun fragment de l'adresse réelle
