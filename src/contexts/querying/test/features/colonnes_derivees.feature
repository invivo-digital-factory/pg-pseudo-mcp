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

  Scénario: Un libellé d'enum converti en texte reste en clair
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT payment_method::text AS moyen, initcap(payment_method::text) AS joli, lower(status::text) AS statut FROM orders WHERE id = 10"
    Alors la requête réussit
    Et la colonne "moyen" vaut "CASH" en clair
    Et la colonne "joli" vaut "Cash" en clair
    Et la colonne "statut" vaut "paid" en clair

  Scénario: Une expression qui ne lit que des colonnes en clair reste en clair
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT concat(status, ' - ', payment_method) AS etiquette, status::text || '/' || placed_at::date AS trace, CASE WHEN amount > 100 THEN 'gros' END AS taille FROM orders WHERE id = 10"
    Alors la requête réussit
    Et la colonne "etiquette" vaut "paid - CASH" en clair
    Et la colonne "trace" vaut "paid/2026-03-01" en clair
    Et la colonne "taille" vaut "gros" en clair

  Scénario: Un comptage converti en texte reste en clair
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT count(*)::text AS total FROM orders"
    Alors la requête réussit
    Et la colonne "total" vaut "4" en clair

  Scénario: Un enum transmis par une CTE reste en clair
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "WITH s AS (SELECT status AS etat FROM orders WHERE id = 11) SELECT upper(etat::text) AS etat FROM s"
    Alors la requête réussit
    Et la colonne "etat" vaut "SHIPPED" en clair

  Plan du Scénario: Une expression qui touche une donnée personnelle ou une fonction inconnue reste pseudonymisée
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "<requete>"
    Alors la requête réussit
    Et la colonne "v" est signalée comme dérivée et pseudonymisée
    Et la réponse ne contient aucun fragment de l'adresse réelle

    Exemples:
      | requete                                                                                                                      |
      | SELECT concat(o.status, ' ', c.nom) AS v FROM orders o JOIN customers c ON c.id = o.customer_id WHERE o.id = 10              |
      | SELECT upper(nom) AS v FROM customers WHERE id = 1                                                                           |
      | SELECT customer_name(customer_id) AS v FROM orders WHERE id = 10                                                             |
      | SELECT (SELECT nom FROM customers WHERE id = 1) AS v FROM orders WHERE id = 10                                               |
      | SELECT v FROM (SELECT nom AS v FROM customers WHERE id = 1 UNION ALL SELECT status::text FROM orders WHERE id = 10) u         |

  Scénario: Un pseudonyme écrit en littéral dans la projection revient tel quel
    Étant donné l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
    Alors la première cellule de la réponse est un pseudonyme
    Quand l'agent affiche ce pseudonyme à côté du statut des commandes filtrées sur ce pseudonyme
    Alors la requête réussit
    Et chaque ligne montre le statut suivi du pseudonyme, en clair
    Et le pseudonyme a été traduit en une valeur réelle côté serveur
