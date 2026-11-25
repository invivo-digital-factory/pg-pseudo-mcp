# language: fr
@querying
Fonctionnalité: Suivi d'un client par son pseudonyme
  L'agent retrouve les données d'un client sans jamais connaître ses valeurs réelles : il réutilise dans un filtre le pseudonyme qu'il a lu,
  et le serveur le remplace localement par la valeur réelle avant l'envoi, là où il est comparé à une colonne ou à un champ.
  Ailleurs, un pseudonyme reste tel quel et la réponse le signale ; translated_literals compte les traductions.

  Règle: PostgreSQL : un pseudonyme lu dans une réponse retrouve les lignes du client

    @integration
    Scénario: Les commandes d'un client se retrouvent à partir de son adresse pseudonymisée
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
      Alors la première cellule de la réponse est un pseudonyme
      Quand l'agent cherche les commandes du client dont l'adresse est ce pseudonyme
      Alors la requête réussit
      Et la réponse liste les commandes 10 et 11
      Et le pseudonyme a été traduit en une valeur réelle côté serveur

    @integration
    Plan du Scénario: Un pseudonyme de type <type> est traduit, il retrouve le client
      Étant donné l'agent connecté à la base de test
      Quand l'agent lit la colonne "<colonne>" du client 1
      Alors la colonne lue est pseudonymisée comme "<type>"
      Quand l'agent filtre les clients sur ce pseudonyme dans la colonne "<colonne>"
      Alors la réponse contient le client 1
      Et le pseudonyme a été traduit en une valeur réelle côté serveur

      Exemples:
        | colonne   | type      |
        | email     | email     |
        | telephone | phone     |
        | iban      | iban      |
        | siren     | siren     |
        | nir       | nir       |
        | nom       | last_name |

    @integration
    Plan du Scénario: Un pseudonyme de type <type> n'est pas traduit, il ne retrouve personne
      Étant donné l'agent connecté à la base de test
      Quand l'agent lit la colonne "<colonne>" du client 1
      Alors la colonne lue est pseudonymisée comme "<type>"
      Quand l'agent filtre les clients sur ce pseudonyme dans la colonne "<colonne>"
      Alors la réponse compte 0 lignes
      Et aucun pseudonyme n'a été traduit

      Exemples:
        | colonne | type       |
        | prenom  | first_name |
        | ville   | city       |

  Règle: PostgreSQL : un pseudonyme est traduit là où il est comparé à une colonne
    =, <>, IN, = ANY, un cast, LIKE et ILIKE, une clé JSON ou un nombre : la valeur réelle part à la place du pseudonyme.

    Contexte:
      Étant donné le pseudonyme "claire.martin@example.net" désigne la valeur réelle "ylwenna.kervadec@canary-mail.fr"
      Et le pseudonyme "paul.rivet@example.org" désigne la valeur réelle "odrigan.hazvekmor@canary-mail.fr"

    Plan du Scénario: Une comparaison traduit chaque pseudonyme
      Quand la garde SQL examine la requête "<requête>"
      Alors la demande est acceptée
      Et <n> valeurs ont été traduites
      Et la requête envoyée ne contient pas "example."

      Exemples:
        | requête                                                                                                                | n |
        | SELECT o.id FROM orders o JOIN customers c ON c.id = o.customer_id WHERE c.email = 'claire.martin@example.net'         | 1 |
        | SELECT id FROM customers WHERE email IN ('claire.martin@example.net', 'paul.rivet@example.org')                        | 2 |
        | SELECT id FROM customers WHERE email = ANY(ARRAY['claire.martin@example.net', 'paul.rivet@example.org'])               | 2 |
        | SELECT id FROM customers WHERE 'claire.martin@example.net'::text = email OR email = 'paul.rivet@example.org'           | 2 |
        | SELECT CASE WHEN email = 'claire.martin@example.net' THEN 1 END FROM customers WHERE email <> 'paul.rivet@example.org' | 2 |
        | SELECT id FROM customers WHERE profile->'contact'->>'mail' = 'claire.martin@example.net'                               | 1 |

    Scénario: Un pseudonyme dans un motif LIKE est traduit et ses caractères spéciaux échappés
      Étant donné le pseudonyme "anne.lorin@example.net" désigne la valeur réelle "ilo_na%k@canary-mail.fr"
      Quand la garde SQL examine la requête "SELECT id FROM customers WHERE email ILIKE '%anne.lorin@example.net%'"
      Alors 1 valeur a été traduite
      Et la requête envoyée contient "'%ilo\_na\%k@canary-mail.fr%'"

    Scénario: Un pseudonyme numérique est traduit en nombre
      Étant donné le pseudonyme "33639981234" désigne la valeur réelle "33641278395"
      Quand la garde SQL examine la requête "SELECT id FROM customers WHERE telephone_secours = 33639981234"
      Alors 1 valeur a été traduite
      Et la requête envoyée contient "= 33641278395"

    Scénario: Une valeur réelle contenant des guillemets ne peut pas modifier la requête
      Étant donné le pseudonyme "Kalin Torvo" désigne la valeur réelle "x'); DELETE FROM orders; -- \ $$"
      Quand la garde SQL examine la requête "SELECT id FROM orders WHERE titulaire = 'Kalin Torvo'"
      Alors 1 valeur a été traduite
      Et la requête envoyée reste une seule lecture

  Règle: PostgreSQL : hors comparaison, un pseudonyme reste tel quel avec un avertissement
    Projection, fonction, plage ou expression sur la colonne : le pseudonyme n'est pas traduit et la réponse porte pseudonym_not_translated.

    Contexte:
      Étant donné le pseudonyme "claire.martin@example.net" désigne la valeur réelle "ylwenna.kervadec@canary-mail.fr"

    Plan du Scénario: Un pseudonyme hors comparaison n'est pas traduit
      Quand la garde SQL examine la requête "<requête>"
      Alors 0 valeur a été traduite
      Et la requête envoyée ne contient pas "ylwenna"
      Et la demande porte l'avertissement "pseudonym_not_translated"

      Exemples:
        | requête                                                                           |
        | SELECT 'claire.martin@example.net' AS x                                           |
        | SELECT substr('claire.martin@example.net', 1, 6)                                  |
        | SELECT md5('claire.martin@example.net') FROM customers                            |
        | SELECT id FROM customers WHERE email > 'claire.martin@example.net'                |
        | SELECT id FROM customers WHERE email BETWEEN 'claire.martin@example.net' AND 'z'  |
        | SELECT id FROM customers WHERE lower(email) = 'claire.martin@example.net'         |

    Scénario: Un pseudonyme écrit dans la projection n'est pas traduit, même filtré ailleurs dans la requête
      Quand la garde SQL examine la requête "SELECT 'claire.martin@example.net' AS brut FROM customers c WHERE c.email = 'claire.martin@example.net'"
      Alors 1 valeur a été traduite
      Et la requête envoyée contient "SELECT 'claire.martin@example.net' AS brut"
      Et la requête envoyée contient "WHERE c.email = 'ylwenna.kervadec@canary-mail.fr'"

    @integration
    Scénario: Un pseudonyme écrit en littéral dans la projection revient tel quel
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT email FROM customers WHERE id = 1"
      Alors la première cellule de la réponse est un pseudonyme
      Quand l'agent affiche ce pseudonyme à côté du statut des commandes filtrées sur ce pseudonyme
      Alors la requête réussit
      Et chaque ligne montre le statut suivi du pseudonyme, en clair
      Et le pseudonyme a été traduit en une valeur réelle côté serveur

  Règle: PostgreSQL : un pseudonyme inconnu sur ce poste part tel quel et le signale
    Une valeur qui a la forme d'un pseudonyme mais qu'aucune réponse n'a produite ici donne unresolved_pseudonym, plutôt qu'une réponse vide muette.

    Scénario: Un pseudonyme inconnu est laissé avec un avertissement
      Quand la garde SQL examine la requête "SELECT id FROM customers WHERE email = 'nobody.known@example.net'"
      Alors 0 valeur a été traduite
      Et la requête envoyée contient "nobody.known@example.net"
      Et la demande porte l'avertissement "unresolved_pseudonym"

    Scénario: Une valeur réelle qui ressemble à un pseudonyme n'est jamais citée dans un avertissement
      Étant donné le pseudonyme "anne.lorin7@example.net" désigne la valeur réelle "qa@example.org"
      Quand la garde SQL examine la requête "SELECT id FROM customers WHERE email = 'anne.lorin7@example.net'"
      Alors 1 valeur a été traduite
      Et la demande ne porte aucun avertissement

    @integration
    Scénario: Un pseudonyme inconnu donne une réponse vide signalée
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT id FROM customers WHERE email = 'nobody.here@example.net'"
      Alors la requête réussit
      Et la réponse compte 0 lignes
      Et aucun pseudonyme n'a été traduit
      Et la réponse porte l'avertissement "unresolved_pseudonym"

  Règle: MongoDB : un pseudonyme est traduit là où il est comparé à un champ
    {champ: valeur}, $eq, $ne, $in, $nin, $all, $elemMatch, et $eq, $ne ou $in dans $expr entre un chemin et une valeur,
    dans le filtre de find comme dans chaque $match, sous-pipelines compris.

    Contexte:
      Étant donné l'agent connecté à une base MongoDB
      Et le pseudonyme "noemie.vasseur@example.net" désigne la valeur réelle "ylwenna.kervadec@canary-mail.fr"

    Plan du Scénario: Un pseudonyme comparé à un champ est traduit avant l'envoi
      Quand l'agent cherche avec le filtre suivant :
        """
        <filtre>
        """
      Alors la demande est acceptée
      Et 1 valeur a été traduite
      Et le filtre envoyé ne contient plus le pseudonyme

      Exemples:
        | filtre                                                                  |
        | {"customer.email": "noemie.vasseur@example.net"}                        |
        | {"customer.email": {"$ne": "noemie.vasseur@example.net"}}               |
        | {"customer.email": {"$in": ["noemie.vasseur@example.net"]}}             |
        | {"customer.email": {"$nin": ["noemie.vasseur@example.net"]}}            |
        | {"emails": {"$all": ["noemie.vasseur@example.net"]}}                    |
        | {"contacts": {"$elemMatch": {"email": "noemie.vasseur@example.net"}}}   |
        | {"$expr": {"$eq": ["$contact.email", "noemie.vasseur@example.net"]}}    |

    Scénario: Un pseudonyme dans le $match d'un sous-pipeline est traduit
      Quand l'agent lance l'agrégation suivante :
        """
        [{"$lookup": {"from": "orders", "pipeline": [{"$match": {"customer.email": "noemie.vasseur@example.net"}}], "as": "commandes"}}]
        """
      Alors la demande est acceptée
      Et 1 valeur a été traduite

    Scénario: Un pseudonyme numérique redevient un nombre quand la valeur réelle est en chiffres
      Étant donné le pseudonyme "734918265" désigne la valeur réelle "418273652"
      Quand l'agent cherche avec le filtre suivant :
        """
        {"phone": 734918265}
        """
      Alors la demande est acceptée
      Et le filtre envoyé vaut :
        """
        {"phone": 418273652}
        """

    @integration
    Scénario: Un client suivi d'une collection à l'autre par le pseudonyme de son email
      Étant donné la connexion MongoDB de test à travers le bastion
      Quand l'agent cherche dans "customers" le document d'identifiant "65a000000000000000000001"
      Alors le champ "contact.email" est pseudonymisé comme "email"
      Quand l'agent cherche dans "orders" les documents dont "customer.email" vaut ce pseudonyme
      Alors la réponse compte 2 documents
      Et un pseudonyme a été retraduit

  Règle: MongoDB : hors comparaison, un pseudonyme reste tel quel avec un avertissement

    Contexte:
      Étant donné l'agent connecté à une base MongoDB
      Et le pseudonyme "noemie.vasseur@example.net" désigne la valeur réelle "ylwenna.kervadec@canary-mail.fr"

    Plan du Scénario: Un pseudonyme hors comparaison reste tel quel avec un avertissement
      Quand l'agent cherche avec le filtre suivant :
        """
        <filtre>
        """
      Alors la demande est acceptée
      Et 0 valeur a été traduite
      Et la demande porte l'avertissement "pseudonym_not_translated"

      Exemples:
        | filtre                                                                          |
        | {"contact.email": {"$regex": "^noemie.vasseur@example.net"}}                    |
        | {"contact.email": {"$gt": "noemie.vasseur@example.net"}}                        |
        | {"$expr": {"$eq": [{"$concat": ["$a", "noemie.vasseur@example.net"]}, "x"]}}    |

  Règle: MongoDB : un pseudonyme inconnu sur ce poste part tel quel et le signale

    Contexte:
      Étant donné l'agent connecté à une base MongoDB

    Scénario: Un pseudonyme inconnu sur ce poste est comparé tel quel
      Quand l'agent cherche avec le filtre suivant :
        """
        {"email": "inconnu.total@example.org"}
        """
      Alors la demande est acceptée
      Et 0 valeur a été traduite
      Et la demande porte l'avertissement "unresolved_pseudonym"
