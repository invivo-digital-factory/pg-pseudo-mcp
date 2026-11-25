# language: fr
@querying
Fonctionnalité: Erreurs des outils
  Une erreur rendue à l'agent est un objet structuré : un code, une catégorie, un message, un indice pour s'en sortir et si l'appel peut être retenté.
  Les valeurs qu'un serveur cite dans ses messages d'erreur sont pseudonymisées comme les résultats : une erreur ne laisse jamais passer une valeur réelle.

  Règle: Une connexion inconnue est signalée avec les connexions disponibles

    Scénario: Une connexion absente de la liste est refusée
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent exécute "SELECT 1" sur la connexion "inconnue"
      Alors l'outil est refusé avec le code "unknown_connection"
      Et l'erreur est de catégorie "unknown_connection"
      Et l'indice de l'erreur contient "canary-local"

  Règle: Chaque outil de lecture refuse l'autre moteur en nommant le bon outil
    query et explain lisent PostgreSQL, find et aggregate lisent MongoDB.

    Plan du Scénario: <outil> sur une connexion PostgreSQL renvoie vers query
      Étant donné une connexion PostgreSQL vers une base injoignable
      Quand l'agent appelle <outil> sur la connexion PostgreSQL
      Alors l'outil est refusé et renvoie vers "query"

      Exemples:
        | outil     |
        | find      |
        | aggregate |

    @integration
    Plan du Scénario: <outil> sur une connexion MongoDB renvoie vers find ou aggregate
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent appelle <outil> sur la connexion MongoDB
      Alors l'outil est refusé et renvoie vers "find"

      Exemples:
        | outil   |
        | query   |
        | explain |

  Règle: Sans magasin de pseudonymes utilisable, tout outil de données refuse sans rien lire
    Le serveur reste actif, mais ne rend aucune donnée qu'il ne saurait pas pseudonymiser.

    Plan du Scénario: <outil> refuse quand le magasin de pseudonymes est inutilisable
      Étant donné une connexion PostgreSQL dont le magasin de pseudonymes est inutilisable
      Quand l'agent appelle <outil> sur la connexion PostgreSQL
      Alors l'outil est refusé avec le code "pseudonym_store_unavailable"
      Et l'erreur est de catégorie "withheld_fail_closed"
      Et l'erreur ne peut pas être retentée

      Exemples:
        | outil          |
        | query          |
        | explain        |
        | list_tables    |
        | describe_table |
        | search_schema  |
        | list_databases |

  @integration
  Règle: PostgreSQL : une erreur du serveur cite le pseudonyme, jamais la valeur réelle

    Scénario: Une conversion en erreur cite le pseudonyme filtré
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
      Alors la première cellule de la réponse est un pseudonyme
      Quand l'agent convertit ce pseudonyme en nombre
      Alors l'outil est refusé avec le code "sql_error"
      Et l'erreur porte le code SQLSTATE "22P02"
      Et la réponse cite le pseudonyme
      Et la réponse ne contient aucune valeur canari

    Plan du Scénario: Une erreur de calcul sur une donnée personnelle ne cite aucune valeur réelle
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "<requête>"
      Alors l'outil est refusé avec le code "sql_error"
      Et la réponse ne contient aucune valeur canari

      Exemples:
        | requête                                                  |
        | SELECT email::int FROM customers WHERE id = 1            |
        | SELECT nom::date FROM customers WHERE id = 1             |
        | SELECT iban::numeric FROM customers WHERE id = 1         |

  @integration
  Règle: MongoDB : une erreur du serveur est pseudonymisée et garde son code MongoDB

    Scénario: AE9, l'erreur de conversion du serveur ne cite jamais le nom réel
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent agrège "customers" avec [{"$project": {"n": {"$toInt": "$lastName"}}}]
      Alors l'outil est refusé avec le code "mongodb_error"
      Et l'erreur porte un code_name MongoDB et aucun SQLSTATE
      Et la réponse ne contient pas "Kervadec"
      Et la réponse ne contient aucune valeur canari

    Scénario: Une erreur du serveur ne ferme pas la session
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent agrège "customers" avec [{"$project": {"n": {"$toInt": "$lastName"}}}]
      Alors l'outil est refusé avec le code "mongodb_error"
      Quand l'agent cherche dans "customers" le document d'identifiant "65a000000000000000000001"
      Alors la réponse compte 1 documents
