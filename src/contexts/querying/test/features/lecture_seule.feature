# language: fr
@integration @querying
Fonctionnalité: Lecture seule
  L'agent lit la base sans jamais pouvoir la modifier, ni par une écriture, ni par un effet de bord.

  Contexte:
    Étant donné l'agent connecté à la base de test

  Scénario: Un SELECT rend les lignes demandées
    Quand l'agent exécute la requête "SELECT id FROM orders ORDER BY id"
    Alors la requête réussit
    Et la réponse compte 4 lignes

  Scénario: Un DELETE est refusé et ne change rien
    Quand l'agent exécute la requête "DELETE FROM orders"
    Alors la requête est refusée parce qu'elle n'est pas en lecture seule
    Quand l'agent exécute la requête "SELECT count(*) FROM orders"
    Alors la requête réussit
    Et la première cellule de la réponse vaut 4

  Plan du Scénario: Une écriture déguisée est refusée
    Quand l'agent exécute la requête "<requête>"
    Alors la requête est refusée parce qu'elle n'est pas en lecture seule

    Exemples:
      | requête                                                                            |
      | WITH x AS (UPDATE customers SET nom = 'x' RETURNING id) SELECT * FROM x            |
      | SELECT * FROM LATERAL pg_terminate_backend(1) t                                    |
