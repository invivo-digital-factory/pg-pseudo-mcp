# language: fr
@anonymization
Fonctionnalité: Détection des valeurs personnelles dans une colonne inconnue
  Quand le nom d'une colonne ne dit rien de son contenu, chaque valeur est lue par des détecteurs : adresse e-mail,
  téléphone français, IBAN, SIREN, SIRET, NIR, carte, adresse IP, adresse postale française (numéro et type de voie),
  nom précédé d'une civilité ou connu du dictionnaire des noms. Dans un texte, seul le fragment détecté est remplacé ;
  une valeur courte qui a la forme d'un nom est remplacée en entier. La réponse signale chaque colonne pseudonymisée avec
  son type et sa raison : detected (un détecteur), dictionary (le nom de la colonne), default (la forme d'un nom),
  derived (une colonne calculée), policy (la politique de colonnes).

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Chaque détecteur reconnaît son type de valeur au milieu d'un texte libre

    Plan du Scénario: Un fragment de type <type> est remplacé au milieu d'un commentaire
      Quand l'agent lit la valeur "<texte>" dans la colonne "commentaire"
      Alors la colonne est signalée "<type>" pour la raison "detected"
      Et la valeur rendue ne contient plus "<fragment>"
      Et la valeur rendue contient "<texte conservé>"

      Exemples:
        | type        | texte                                                   | fragment                          | texte conservé  |
        | email       | écrire à ylwenna.kervadec@canary-mail.fr demain         | ylwenna.kervadec@canary-mail.fr   | écrire à        |
        | phone       | rappeler au 06 41 27 83 95 demain                       | 06 41 27 83 95                    | rappeler au     |
        | phone       | rappeler au +33 6 41 27 83 95 demain                    | 6 41 27 83 95                     | rappeler au     |
        | phone       | rappeler au 06.41.27.83.95 demain                       | 06.41.27.83.95                    | rappeler au     |
        | iban        | virement FR76 3000 4000 0312 3456 7890 143 reçu         | 3000 4000 0312                    | virement        |
        | siren       | société 418 273 652 inscrite                            | 418 273 652                       | société         |
        | siret       | établissement 418 273 652 10001 ouvert                  | 418 273 652 10001                 | établissement   |
        | nir         | assuré 2851175123456 22 couvert                         | 2851175123456 22                  | assuré          |
        | card_number | carte 4970 1012 3456 7810 refusée                       | 4970 1012 3456 7810               | carte           |
        | ip_address  | connexion depuis 192.168.10.4 hier                      | 192.168.10.4                      | connexion depuis |
        | ip_address  | connexion depuis fd12:3456:789a::1 hier                 | fd12:3456:789a::1                 | connexion depuis |
        | address     | livraison au 12 rue des Lilas, merci                    | 12 rue des Lilas                  | livraison au    |
        | last_name   | appel de M. Brizolier pour le colis                     | Brizolier                         | appel de M.     |
        | person_name | voir avec Mme Ylwenna Kervadec pour le colis            | Ylwenna Kervadec                  | voir avec Mme   |

  Règle: Un numéro n'est détecté que s'il est valide : clé de contrôle juste, octets d'adresse IP possibles

    Plan du Scénario: "<texte>" reste en clair car <raison>
      Quand l'agent lit la valeur "<texte>" dans la colonne "commentaire"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

      Exemples:
        | texte                                           | raison                                |
        | virement FR76 3000 6000 0112 3456 7890 188 reçu | le modulo 97 de l'IBAN est faux       |
        | société 418 273 653 inscrite                    | la clé de Luhn du SIREN est fausse    |
        | assuré 2851175123456 23 couvert                 | la clé du NIR est fausse              |
        | logiciel en version 999.1.2.3                   | 999 n'est pas un octet d'adresse IP   |

  Règle: Quand deux détections se chevauchent, la plus longue l'emporte

    Scénario: Un SIRET n'est pas découpé en SIREN
      Quand l'agent lit la valeur "SIRET 418 273 652 10001" dans la colonne "commentaire"
      Alors la colonne est signalée "siret" pour la raison "detected"
      Et la valeur rendue contient "SIRET "
      Et la valeur rendue ne contient plus "418 273 652"

  Règle: Une valeur courte qui a la forme d'un nom est pseudonymisée par défaut

    Plan du Scénario: "<valeur>" dans une colonne inconnue est remplacé en entier comme <type>
      Quand l'agent lit la valeur "<valeur>" dans la colonne "titulaire"
      Alors la valeur est pseudonymisée
      Et la colonne est signalée "<type>" pour la raison "<raison>"

      Exemples: une forme de nom, connue ou non du dictionnaire des noms
        | valeur                | type        | raison   |
        | Kervadec              | last_name   | default  |
        | Ylwenna Kervadec      | person_name | default  |
        | Ylwenna de Kervadec   | person_name | default  |
        | KERVADEC Ylwenna      | person_name | default  |
        | Y. Kervadec           | person_name | default  |
        | Marie Kervadec        | person_name | detected |

    Plan du Scénario: "<valeur>" n'a pas la forme d'un nom et reste en clair
      Quand l'agent lit la valeur "<valeur>" dans la colonne "titulaire"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

      Exemples: des codes, statuts et dates
        | valeur                    |
        | PAID                      |
        | EN COURS                  |
        | pending                   |
        | en_cours                  |
        | En attente                |
        | FR                        |
        | Li                        |
        | 2026-11-19                |
        | Un Deux Trois Quatre Cinq |

  Règle: Une valeur égale à un libellé d'énumération de la base n'est pas prise pour un nom

    Scénario: "Sepa" est pris pour un nom sans libellé d'énumération
      Quand l'agent lit la valeur "Sepa" dans la colonne "moyen"
      Alors la valeur est pseudonymisée

    Scénario: "Sepa" reste en clair quand la base déclare le libellé "SEPA", quelle que soit la casse
      Étant donné que la base déclare les libellés d'énumération "SEPA, AUTHORIZED"
      Quand l'agent lit la valeur "Sepa" dans la colonne "moyen"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

    Scénario: Un libellé d'énumération ne protège pas une colonne que le dictionnaire déclare personnelle
      Étant donné que la base déclare les libellés d'énumération "KERVADEC"
      Quand l'agent lit la valeur "Kervadec" dans la colonne "nom"
      Alors la valeur est pseudonymisée
      Et la colonne est signalée "last_name" pour la raison "dictionary"
