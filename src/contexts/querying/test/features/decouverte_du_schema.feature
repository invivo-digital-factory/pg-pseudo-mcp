# language: fr
@integration @querying
Fonctionnalité: Découverte du schéma
  Avant d'écrire une requête, l'agent explore la structure : list_tables liste tables, vues et collections, describe_table décrit
  colonnes ou champs, clés et index, search_schema retrouve un nom par sous-chaîne. Ces outils annoncent quelles colonnes ou quels champs
  seront pseudonymisés, mais ne rendent jamais une valeur réelle : ni valeur par défaut, ni valeur échantillonnée, ni donnée de commentaire.

  Règle: PostgreSQL : list_tables rend des entrées compactes, triées, filtrables et paginées

    Contexte:
      Étant donné l'agent connecté à la base de test

    Scénario: Les tables du jeu de données sont listées par ordre de nom
      Quand l'agent liste les tables
      Alors la liste des tables contient "big_numbers", "customers", "notes" et "orders"
      Et la liste des tables est triée par nom
      Et la table "customers" compte 15 colonnes

    Scénario: Un filtre sur le nom ignore la casse
      Quand l'agent liste les tables dont le nom contient "NUM"
      Alors la liste des tables vaut "big_numbers"

    Scénario: Une page de deux tables donne le décalage de la suivante
      Quand l'agent liste 2 tables
      Alors la liste des tables vaut "big_numbers, customers"
      Et la page suivante commence au décalage 2

  Règle: PostgreSQL : describe_table décrit une table sans exposer de valeur

    Contexte:
      Étant donné l'agent connecté à la base de test

    Scénario: Clés étrangères, clé primaire et index d'une table
      Quand l'agent décrit la table "orders"
      Alors la description donne la clé primaire "id"
      Et la description donne la clé étrangère "customer_id" vers "public.customers(id)"
      Et la description donne l'index "orders_customer_id_idx" sur "customer_id"

    Scénario: Les références entrantes d'une table
      Quand l'agent décrit la table "public.customers"
      Alors la table est référencée par "public.orders" sur "customer_id"
      Et la table est référencée par "public.notes" sur "customer_id"

    Plan du Scénario: La colonne <colonne> sera pseudonymisée comme <type>
      Quand l'agent décrit la table "customers"
      Alors la colonne décrite "<colonne>" sera pseudonymisée comme "<type>"

      Exemples:
        | colonne   | type               |
        | email     | email              |
        | nom       | last_name          |
        | prenom    | first_name         |
        | telephone | phone              |
        | ville     | city               |
        | profile   | detected_per_value |

    Scénario: Les colonnes techniques restent en clair
      Quand l'agent décrit la table "customers"
      Alors la colonne décrite "id" ne sera pas pseudonymisée
      Et la colonne décrite "created_at" ne sera pas pseudonymisée

    Scénario: Un commentaire est pseudonymisé et une valeur par défaut n'est jamais rendue
      Quand l'agent décrit la table "customers"
      Alors le commentaire de la colonne "email" commence par "Primary contact"
      Et la réponse ne contient aucune valeur canari
      Et la réponse ne contient pas "2026-01-15"

    Plan du Scénario: Une table inconnue est refusée avec les noms proches
      Quand l'agent décrit la table "<table>"
      Alors l'outil est refusé avec le code "unknown_table"
      Et l'indice de l'erreur contient "<indice>"

      Exemples:
        | table               | indice                      |
        | order               | public.orders               |
        | nope                | public.notes                 |

  Règle: PostgreSQL : search_schema retrouve tables et colonnes par sous-chaîne, sans égard à la casse

    Contexte:
      Étant donné l'agent connecté à la base de test

    Scénario: Une sous-chaîne retrouve les colonnes de plusieurs tables
      Quand l'agent cherche "MAIL" dans le schéma
      Alors la recherche trouve la colonne "email" de "public.customers"
      Et la recherche trouve la colonne "emails_secondaires" de "public.customers"

    Scénario: Une sous-chaîne retrouve une table par son nom
      Quand l'agent cherche "big_num" dans le schéma
      Alors la recherche trouve la table "public.big_numbers"

    Plan du Scénario: Guillemets et jokers SQL sont cherchés comme du texte
      Quand l'agent cherche "<motif>" dans le schéma
      Alors la recherche ne trouve rien

      Exemples:
        | motif               |
        | it's                |
        | %                   |
        | _x'); SELECT 1; --  |

  Règle: MongoDB : les outils de schéma lisent les collections sans rendre de valeur échantillonnée

    Contexte:
      Étant donné la connexion MongoDB de test à travers le bastion

    Scénario: list_tables liste les collections et la vue, jamais une collection système
      Quand l'agent liste les collections
      Alors la liste contient la collection "customers" de type "collection"
      Et la liste contient la collection "orders" de type "collection"
      Et la liste contient la collection "customer_contacts" de type "view"
      Et la liste ne contient aucune collection système

    Scénario: describe_table donne chemins, types et pseudonymisation sans valeur échantillonnée
      Quand l'agent décrit la collection "customers"
      Alors le champ décrit "contact.email" est de type "string" et sera pseudonymisé comme "email"
      Et le champ décrit "lastName" sera pseudonymisé comme "last_name"
      Et le champ décrit "birthDate" est de type "date"
      Et la description donne un index sur "contact.email"
      Et la réponse ne contient aucune valeur canari

    Scénario: describe_table sur une collection absente rend la table inconnue
      Quand l'agent décrit la collection "ghosts"
      Alors l'outil est refusé avec le code "unknown_table"

    Scénario: search_schema retrouve les chemins de champs
      Quand l'agent cherche "mail" dans les collections
      Alors la recherche trouve le champ "contact.email"
      Et la recherche trouve le champ "customer.email"
      Et la réponse ne contient aucune valeur canari
