# language: fr
@querying
Fonctionnalité: Valeurs calculées et lignée
  Une valeur calculée à partir d'une donnée personnelle ne doit jamais en révéler un morceau : le serveur suit la lignée de chaque colonne
  ou champ de sortie jusqu'à ses sources. Une valeur recopiée garde le jugement de sa source ; une valeur calculée ne reste en clair que
  si chaque source lue est en clair et chaque opération est sur une liste qui préserve le clair. Sinon elle est dérivée et pseudonymisée.
  En PostgreSQL une colonne est une source en clair par son type (enum, nombre, date, booléen, uuid) ou par la politique de colonnes ;
  en MongoDB, qui n'a pas de schéma, seul un champ déclaré clear dans la politique de champs l'est.
  Vocabulaire : une sortie de lignée personnelle lit au moins une donnée personnelle ; une sortie propre n'en lit aucune ;
  une sortie de lignée inconnue n'a pas pu être suivie. Un champ dérivé « teinté » est pseudonymisé, un champ dérivé « prouvé en clair » est rendu tel quel.

  Règle: PostgreSQL : un calcul sur une donnée personnelle est dérivé et pseudonymisé

    @integration
    Scénario: Le début d'une adresse découpé en SQL revient pseudonymisé
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT split_part(email, '@', 1) AS debut FROM customers WHERE id = 1"
      Alors la requête réussit
      Et la colonne "debut" est signalée comme dérivée et pseudonymisée
      Et la réponse ne contient aucun fragment de l'adresse réelle

    @integration
    Plan du Scénario: Une expression qui touche une donnée personnelle ou une fonction inconnue reste pseudonymisée
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "<requête>"
      Alors la requête réussit
      Et la colonne "v" est signalée comme dérivée et pseudonymisée
      Et la réponse ne contient aucun fragment de l'adresse réelle

      Exemples:
        | requête                                                                                                                      |
        | SELECT concat(o.status, ' ', c.nom) AS v FROM orders o JOIN customers c ON c.id = o.customer_id WHERE o.id = 10              |
        | SELECT upper(nom) AS v FROM customers WHERE id = 1                                                                           |
        | SELECT customer_name(customer_id) AS v FROM orders WHERE id = 10                                                             |
        | SELECT (SELECT nom FROM customers WHERE id = 1) AS v FROM orders WHERE id = 10                                               |
        | SELECT v FROM (SELECT nom AS v FROM customers WHERE id = 1 UNION ALL SELECT status::text FROM orders WHERE id = 10) u         |
        | SELECT lower(x) AS v FROM (SELECT status::text AS x FROM orders WHERE id = 10 UNION ALL SELECT nom FROM customers WHERE id = 1) u |

    @integration
    Plan du Scénario: Une valeur sérialisée, agrégée ou encodée ne laisse passer aucune valeur réelle
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "<requête>"
      Alors la requête réussit
      Et la réponse ne contient aucune valeur canari

      Exemples:
        | requête                                                    |
        | SELECT to_json(c) AS v FROM customers c WHERE id = 1       |
        | SELECT string_agg(email, ',') AS v FROM customers          |
        | SELECT lower(email) AS v FROM customers WHERE id = 1       |
        | SELECT md5(email) AS v FROM customers WHERE id = 1         |
        | SELECT encode(email::bytea, 'base64') AS v FROM customers WHERE id = 1 |
        | SELECT profile AS v FROM customers WHERE id = 1            |
        | SELECT emails_secondaires AS v FROM customers WHERE id = 1 |

  Règle: PostgreSQL : un calcul qui ne lit que des colonnes en clair par leur type reste en clair
    Enum, nombre, date : à travers les casts, concat, ||, CASE, les comptages, les CTE et les sous-requêtes de FROM.

    @integration
    Scénario: Un libellé d'enum converti en texte reste en clair
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT payment_method::text AS moyen, initcap(payment_method::text) AS joli, lower(status::text) AS statut FROM orders WHERE id = 10"
      Alors la requête réussit
      Et la colonne "moyen" vaut "CASH" en clair
      Et la colonne "joli" vaut "Cash" en clair
      Et la colonne "statut" vaut "paid" en clair

    @integration
    Scénario: Une expression qui ne lit que des colonnes en clair reste en clair
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT concat(status, ' - ', payment_method) AS etiquette, status::text || '/' || placed_at::date AS trace, CASE WHEN amount > 100 THEN 'gros' END AS taille FROM orders WHERE id = 10"
      Alors la requête réussit
      Et la colonne "etiquette" vaut "paid - CASH" en clair
      Et la colonne "trace" vaut "paid/2026-03-01" en clair
      Et la colonne "taille" vaut "gros" en clair

    @integration
    Scénario: Un comptage converti en texte reste en clair
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT count(*)::text AS total FROM orders"
      Alors la requête réussit
      Et la colonne "total" vaut "4" en clair

    @integration
    Scénario: Un enum transmis par une CTE reste en clair
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "WITH s AS (SELECT status AS etat FROM orders WHERE id = 11) SELECT upper(etat::text) AS etat FROM s"
      Alors la requête réussit
      Et la colonne "etat" vaut "SHIPPED" en clair

    @integration
    Scénario: Une même colonne lue deux fois garde un indicateur par colonne
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT a.email, b.email, a.id FROM customers a JOIN customers b ON a.id = b.id ORDER BY a.id"
      Alors la requête réussit
      Et les colonnes 1 et 2 sont pseudonymisées comme "email" par le dictionnaire
      Et la colonne 3 n'est pas pseudonymisée

  Règle: PostgreSQL : la lignée suit chaque colonne de sortie jusqu'à ses sources
    Elle traverse alias, CTE, sous-requêtes et branches d'UNION ; une étoile ou une ligne entière ne prouvent rien.

    Plan du Scénario: La sortie <position> de la requête est de lignée <lignée>
      Quand la garde SQL examine la requête "<requête>"
      Alors la sortie <position> est de lignée <lignée>

      Exemples:
        | requête                                                                                       | position | lignée      |
        | SELECT upper(email) AS v, id + 1 AS n FROM customers                                          | 1        | personnelle |
        | SELECT upper(email) AS v, id + 1 AS n FROM customers                                          | 2        | propre     |
        | WITH c AS (SELECT email AS e FROM customers) SELECT lower(e) FROM c                           | 1        | personnelle |
        | SELECT 'x' UNION ALL SELECT split_part(email, '@', 1) FROM customers                          | 1        | personnelle |
        | SELECT to_json(c) FROM customers c                                                            | 1        | personnelle |
        | SELECT * FROM customers                                                                       | 1        | inconnue   |

  Règle: MongoDB : un champ recopié ou renommé garde la classification de sa source
    Projection, clé de regroupement, $first, $push, $unwind, $lookup ou $replaceRoot sur un chemin, ou un élément choisi dans un tableau
    par $arrayElemAt, $first ou $last :
    le champ de sortie est jugé comme le champ d'origine dans sa collection d'origine. Un document intermédiaire qu'une projection d'inclusion
    ne fait que traverser vient lui aussi de son chemin d'origine : chacun de ses champs est jugé pour lui-même.

    Contexte:
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.customers" = ["status", "amount"]
        """
      Quand la politique de colonnes est chargée

    Scénario: Une projection renomme, recopie et calcule
      Quand l'agrégation suivante part sur "shop.customers" :
        """
        [{"$project": {"who": {"$concat": ["$firstName", " ", "$lastName"]}, "mail": "$contact.email", "status": 1}}]
        """
      Alors le champ de sortie "who" est dérivé et teinté
      Et le champ de sortie "mail" vient du champ "contact.email" de "shop.customers"
      Et le champ de sortie "status" vient du champ "status" de "shop.customers"
      Et le champ "status" de "shop.customers" est en clair

    Plan du Scénario: Le champ de sortie <champ> vient du champ <source> de <collection>
      Quand l'agrégation <pipeline> part sur "shop.customers"
      Alors le champ de sortie "<champ>" vient du champ "<source>" de "<collection>"

      Exemples:
        | pipeline                                                                                                       | champ        | source        | collection     |
        | [{"$group": {"_id": "$status", "total": {"$sum": "$amount"}}}]                                                 | _id          | status        | shop.customers |
        | [{"$group": {"_id": null, "emails": {"$push": "$email"}}}]                                                     | emails       | email         | shop.customers |
        | [{"$group": {"_id": null, "first": {"$first": {"name": "$name"}}}}]                                            | first.name   | name          | shop.customers |
        | [{"$unwind": {"path": "$orders", "includeArrayIndex": "position"}}]                                            | orders.total | orders.total  | shop.customers |
        | [{"$lookup": {"from": "users", "localField": "buyerId", "foreignField": "_id", "as": "buyer"}}]                | buyer.email  | email         | shop.users     |
        | [{"$lookup": {"from": "users", "localField": "buyerId", "foreignField": "_id", "as": "buyer"}}]                | name         | name          | shop.customers |
        | [{"$replaceRoot": {"newRoot": "$contact"}}]                                                                    | email        | contact.email | shop.customers |
        | [{"$replaceWith": "$contact"}]                                                                                 | email        | contact.email | shop.customers |
        | [{"$match": {"status": "PAID"}}, {"$sort": {"createdAt": -1}}, {"$limit": 5}]                                  | contact.email | contact.email | shop.customers |
        | [{"$sortByCount": "$status"}]                                                                                  | _id          | status        | shop.customers |
        | [{"$project": {"attributes.level.label": 1, "attributes.size.label": 1}}]                                     | attributes   | attributes    | shop.customers |
        | [{"$project": {"attributes.level.label": 1, "attributes.size.label": 1}}]                                     | attributes.level | attributes.level | shop.customers |
        | [{"$project": {"level": {"$arrayElemAt": ["$levels.label", 0]}}}]                                             | level        | levels.label  | shop.customers |
        | [{"$project": {"mail": {"$first": "$contacts.email"}}}]                                                        | mail         | contacts.email | shop.customers |
        | [{"$project": {"mail": {"$last": ["$contacts.email"]}}}]                                                       | mail         | contacts.email | shop.customers |
        | [{"$project": {"main": {"$arrayElemAt": ["$levels", -1]}}}]                                                    | main.label   | levels.label  | shop.customers |

  Règle: MongoDB : un champ calculé n'est en clair que si tout ce qu'il lit est déclaré clear et chaque opérateur préserve le clair
    Ici status et amount sont déclarés clear pour shop.customers. Préservent le clair : $concat, $toLower, $toUpper, $toString, $trim, $cond, $ifNull,
    $switch, l'arithmétique, les comparaisons, les opérateurs de dates, $sum, $count, $avg, $min et $max ; $substrCP n'en fait pas partie.
    Un littéral, un comptage ou un index de tableau sont des constantes en clair.

    Contexte:
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.customers" = ["status", "amount"]
        """
      Quand la politique de colonnes est chargée

    Plan du Scénario: Le champ calculé <champ> est dérivé et <jugement>
      Quand l'agrégation <pipeline> part sur "shop.customers"
      Alors le champ de sortie "<champ>" est dérivé et <jugement>

      Exemples:
        | pipeline                                                                                       | champ    | jugement          |
        | [{"$group": {"_id": "$status", "total": {"$sum": "$amount"}}}]                                 | total    | prouvé en clair   |
        | [{"$group": {"_id": "$status", "oldest": {"$max": "$birthDate"}}}]                             | oldest   | teinté            |
        | [{"$group": {"_id": null, "orders": {"$sum": 1}}}]                                              | orders   | prouvé en clair   |
        | [{"$project": {"loud": {"$toUpper": "$status"}}}]                                              | loud     | prouvé en clair   |
        | [{"$project": {"start": {"$substrCP": ["$email", 0, 3]}}}]                                     | start    | teinté            |
        | [{"$project": {"start": {"$substrCP": ["$status", 0, 3]}}}]                                    | start    | teinté            |
        | [{"$project": {"label": {"$cond": [{"$eq": ["$status", "PAID"]}, "$status", "$email"]}}}]      | label    | teinté            |
        | [{"$set": {"s": {"$substrCP": ["$email", 0, 3]}}}, {"$project": {"loud": {"$toUpper": "$s"}}}] | loud     | teinté            |
        | [{"$project": {"label": "VIP"}}]                                                               | label    | prouvé en clair   |
        | [{"$count": "n"}]                                                                              | n        | prouvé en clair   |
        | [{"$unwind": {"path": "$orders", "includeArrayIndex": "position"}}]                            | position | prouvé en clair   |
        | [{"$project": {"at": "$$NOW"}}]                                                                | at       | prouvé en clair   |
        | [{"$set": {"card": {"$mergeObjects": ["$status", "$amount"]}}}, {"$set": {"card.extra": 1}}]   | card     | teinté            |
        | [{"$project": {"user": {"$arrayElemAt": [{"$split": ["$email", "@"]}, 0]}}}]                   | user     | teinté            |
        | [{"$project": {"user": {"$first": {"$split": ["$email", "@"]}}}}]                              | user     | teinté            |
        | [{"$project": {"pick": {"$arrayElemAt": ["$status", {"$strLenCP": "$email"}]}}}]               | pick     | teinté            |

  Règle: MongoDB : ce que la lignée ne suit pas est inconnu, donc teinté
    $$ROOT et les variables, $objectToArray, les noms de champ calculés, $facet, $unionWith et les étapes non suivies donnent un champ dérivé.

    Plan du Scénario: Ce que la lignée ne suit pas est inconnu et teinté
      Quand l'agrégation <pipeline> part sur "shop.customers"
      Alors le champ de sortie "<champ>" est de lignée inconnue
      Et le champ de sortie "<champ>" est dérivé et teinté

      Exemples:
        | pipeline                                                                                     | champ           |
        | [{"$project": {"whole": "$$ROOT"}}]                                                          | whole.email     |
        | [{"$replaceRoot": {"newRoot": {"$mergeObjects": ["$contact", "$address"]}}}]                | email           |
        | [{"$project": {"pairs": {"$objectToArray": "$contact"}}}]                                    | pairs           |
        | [{"$project": {"picked": {"$getField": {"field": "$label", "input": "$contact"}}}}]          | picked          |
        | [{"$facet": {"byStatus": [{"$project": {"status": 1}}]}}]                                    | byStatus.status |
        | [{"$unionWith": {"coll": "prospects"}}]                                                      | email           |
        | [{"$unset": ["email"]}]                                                                      | email           |
        | [{"$fill": {"output": {"phone": {"value": "none"}}}}]                                        | phone           |
