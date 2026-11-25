# language: fr
@anonymization
Fonctionnalité: Nombres et dates
  Un nombre ou une date n'est personnel que si sa colonne le dit : un montant, un identifiant ou une date de création
  restent en clair, un téléphone stocké en entier ou une date de naissance sont pseudonymisés. Une date de naissance
  est décalée d'un nombre entier de semaines, pour garder un âge plausible sans révéler le jour réel.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Un nombre n'est pseudonymisé que si le nom de sa colonne le désigne comme personnel

    Plan du Scénario: Le nombre <valeur> de la colonne "<colonne>" est pseudonymisé comme <type> et reste un nombre
      Quand l'agent lit "<valeur>" dans la colonne "<colonne>" de type "<type sql>"
      Alors la valeur est pseudonymisée
      Et la valeur rendue est un nombre
      Et la colonne est signalée "<type>" pour la raison "dictionary"

      Exemples:
        | colonne           | type sql | valeur      | type        |
        | telephone_secours | int8     | 33641278395 | phone       |
        | siren             | int8     | 418273652   | siren       |
        | code_postal       | int4     | 29280       | postal_code |

    Plan du Scénario: Le nombre <valeur> de la colonne technique "<colonne>" reste en clair
      Quand l'agent lit "<valeur>" dans la colonne "<colonne>" de type "<type sql>"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

      Exemples:
        | colonne  | type sql | valeur      |
        | amount   | int8     | 33641278395 |
        | quantity | int8     | 418273652   |
        | id       | int4     | 29280       |

    Scénario: Un téléphone rendu avec des décimales nulles reste reconnu comme téléphone
      Quand l'agent lit "33641278395.0000" dans la colonne "telephone_secours" de type "numeric"
      Alors la valeur est pseudonymisée
      Et la colonne est signalée "phone" pour la raison "dictionary"

  Règle: Les types neutres restent en clair, même sous un nom de colonne personnel

    Plan du Scénario: La colonne "<colonne>" de type <type sql> garde "<valeur>" en clair
      Quand l'agent lit "<valeur>" dans la colonne "<colonne>" de type "<type sql>"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

      Exemples:
        | colonne       | type sql    | valeur                               |
        | nom           | enum        | Kervadec                             |
        | email         | uuid        | 3b241101-e2bb-4255-8caf-4136c566a962 |
        | email_sent_at | timestamptz | 2026-03-02 09:15:00+00               |
        | email_valide  | bool        | true                                 |

  Règle: Une adresse IP typée inet est toujours pseudonymisée, quel que soit le nom de sa colonne

    Scénario: Une adresse inet dans une colonne au nom neutre reçoit une adresse de documentation
      Quand l'agent lit "192.168.10.4" dans la colonne "origine" de type "inet"
      Alors le pseudonyme est une adresse des plages IPv4 de documentation
      Et la colonne est signalée "ip_address" pour la raison "detected"

  Règle: Une date de naissance est décalée d'un nombre entier de semaines et garde son format

    Plan du Scénario: La date de naissance "<valeur>" de type <type sql> est décalée et garde le format <format>
      Quand l'agent lit "<valeur>" dans la colonne "<colonne>" de type "<type sql>"
      Alors la valeur rendue est la même date décalée d'un nombre entier de semaines, de 1 à 52
      Et le pseudonyme a la présentation "<format>"
      Et la colonne est signalée "birth_date" pour la raison "dictionary"

      Exemples:
        | colonne        | type sql  | valeur              | format              |
        | date_naissance | date      | 1985-03-12          | xxxx-xx-xx          |
        | date_naissance | timestamp | 1985-03-12 00:00:00 | xxxx-xx-xx xx:xx:xx |
        | date_naissance | text      | 12/03/1985          | xx/xx/xxxx          |
        | anniversaire   | date      | 1999-12-31          | xxxx-xx-xx          |

    Scénario: Une date de naissance impossible est remplacée par une date plausible
      Quand l'agent lit "1985-02-30" dans la colonne "date_naissance" de type "text"
      Alors la valeur rendue est une date ISO entre 1940 et 2005

    Scénario: Une date de création n'est jamais décalée
      Quand l'agent lit "1985-03-12" dans la colonne "created_at" de type "date"
      Alors la valeur reste en clair
