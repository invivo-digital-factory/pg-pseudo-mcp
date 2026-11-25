# language: fr
@anonymization
Fonctionnalité: Cohérence des pseudonymes et traduction en retour
  L'agent est le client (un assistant de code) qui interroge la base à travers le serveur ; le poste est la machine du
  développeur, qui garde un secret. Sur un poste, une valeur personnelle, écrite à l'identique et du même type, reçoit
  toujours le même pseudonyme jusqu'à la purge. Pour une liste fixe de types, le poste sait traduire un pseudonyme en
  retour vers la valeur réelle : c'est ce qui permet à l'agent de filtrer sur un pseudonyme qu'il a lu.
  Les valeurs « canari » des exemples sont inventées pour ne désigner personne.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: La même valeur reçoit le même pseudonyme, même après un redémarrage du serveur

    Scénario: La même adresse e-mail donne le même pseudonyme, sur un domaine réservé
      Quand l'agent obtient deux fois le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Alors les deux pseudonymes sont identiques
      Et le pseudonyme n'est pas l'adresse réelle
      Et le pseudonyme est sur un domaine réservé

    Plan du Scénario: Une valeur de type <type> garde son pseudonyme d'un démarrage à l'autre
      Quand le serveur démarre deux fois et donne chaque fois le pseudonyme de type "<type>" de "<valeur>"
      Alors les deux pseudonymes sont identiques

      Exemples:
        | type        | valeur                            |
        | email       | ylwenna.kervadec@canary-mail.fr   |
        | phone       | 06 41 27 83 95                    |
        | person_name | Ylwenna Kervadec                  |
        | first_name  | Ylwenna                           |
        | city        | Plouzané                          |
        | birth_date  | 1985-03-12                        |

  Règle: Une valeur est reconnue à son écriture exacte et à son type

    Plan du Scénario: "<première>" et "<seconde>" de type <type> reçoivent deux pseudonymes différents
      Quand l'agent obtient le pseudonyme de type "<type>" de "<première>"
      Et l'agent obtient le pseudonyme de type "<type>" de "<seconde>"
      Alors le pseudonyme 2 diffère du pseudonyme 1

      Exemples:
        | type      | première       | seconde    |
        | last_name | Kervadec       | KERVADEC   |
        | phone     | 06 41 27 83 95 | 0641278395 |

  Règle: Deux valeurs différentes d'un type traduisible ne partagent jamais un pseudonyme

    Scénario: Cent adresses e-mail différentes reçoivent cent pseudonymes différents
      Quand l'agent obtient le pseudonyme de 100 adresses e-mail canari différentes
      Alors les 100 pseudonymes sont tous différents

  Règle: Les pseudonymes dépendent du secret du poste

    Plan du Scénario: Deux postes donnent deux pseudonymes différents à la même valeur de type <type>
      Étant donné un second poste avec son propre répertoire de données
      Quand chaque poste donne le pseudonyme de type "<type>" de "<valeur>"
      Alors les deux pseudonymes sont différents

      Exemples:
        | type        | valeur                          |
        | email       | ylwenna.kervadec@canary-mail.fr |
        | person_name | Ylwenna Kervadec                |
        | iban        | FR76 3000 4000 0312 3456 7890 143 |

  Règle: Seule une liste fixe de types se traduit en retour, et seulement sur le poste qui a émis le pseudonyme

    Scénario: Un pseudonyme se traduit en retour vers la valeur réelle, en local
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et l'agent traduit ce pseudonyme
      Alors la traduction rend l'adresse "ylwenna.kervadec@canary-mail.fr"

    Plan du Scénario: Un pseudonyme de type <type> se traduit en retour : <traduit>
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Et l'agent traduit ce pseudonyme
      Alors le pseudonyme se traduit en retour : <traduit>

      Exemples: les types traduits en retour, que l'agent peut réutiliser dans un filtre
        | type        | valeur                            | traduit |
        | email       | ylwenna.kervadec@canary-mail.fr   | oui     |
        | phone       | 06 41 27 83 95                    | oui     |
        | iban        | FR76 3000 4000 0312 3456 7890 143 | oui     |
        | siren       | 418273652                         | oui     |
        | siret       | 41827365210001                    | oui     |
        | nir         | 2851175123456 22                  | oui     |
        | card_number | 4970 1012 3456 7810               | oui     |
        | ip_address  | 192.168.10.4                      | oui     |
        | person_name | Ylwenna Kervadec                  | oui     |
        | last_name   | Hazvekmor                         | oui     |

      Exemples: les types jamais traduits en retour, que l'agent ne doit pas utiliser comme filtre
        | type        | valeur           | traduit |
        | first_name  | Ylwenna          | non     |
        | address     | 12 rue des Lilas | non     |
        | city        | Plouzané         | non     |
        | postal_code | 29280            | non     |
        | civility    | Mme              | non     |
        | gender      | F                | non     |
        | birth_date  | 1985-03-12       | non     |

    Plan du Scénario: Une valeur qui n'a jamais été émise comme pseudonyme ne se traduit pas
      Quand l'agent traduit "<texte>"
      Alors la traduction ne rend aucune valeur réelle

      Exemples:
        | texte                           |
        | nobody.seen@example.net         |
        | ylwenna.kervadec@canary-mail.fr |

    Scénario: Après une purge, un ancien pseudonyme ne se traduit plus
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le développeur purge la table de correspondance
      Et l'agent traduit ce pseudonyme
      Alors la traduction ne rend aucune valeur réelle

  Règle: Un pseudonyme se reconnaît à sa forme, même quand ce poste ne l'a pas émis, pour avertir l'agent qui le réutilise

    Plan du Scénario: "<texte>" <verdict>
      Quand l'agent traduit "<texte>"
      Alors la forme reconnue est "<type>"

      Exemples: les espaces réservés aux pseudonymes
        | texte                     | type       | verdict                                          |
        | nobody.seen42@example.net | email      | est une adresse sur un domaine réservé           |
        | 06 39 98 12 34            | phone      | est un numéro de mobile fictif                   |
        | +33 1 99 00 12 34         | phone      | est un numéro fixe fictif                        |
        | 198.51.100.23             | ip_address | est une adresse IPv4 de documentation            |
        | 2001:db8::17              | ip_address | est une adresse IPv6 de documentation            |

      Exemples: les valeurs réelles
        | texte                           | type  | verdict                       |
        | ylwenna.kervadec@canary-mail.fr | aucun | n'est pas une forme réservée  |
        | 06 41 27 83 95                  | aucun | n'est pas une forme réservée  |
        | 192.168.10.4                    | aucun | n'est pas une forme réservée  |

  Règle: Une valeur réelle que l'agent a obtenue en filtrant sur un pseudonyme revient sous ce pseudonyme

    Scénario: Le nom traduit depuis un filtre est remplacé par le pseudonyme du filtre dans la réponse
      Étant donné que l'agent a filtré sur le pseudonyme de type "last_name" de "Kervadec"
      Quand l'agent lit le texte libre "rappeler Kervadec demain"
      Alors la valeur rendue est "rappeler {pseudonyme} demain"

    Scénario: Le nom traduit est remplacé quelle que soit sa casse
      Étant donné que l'agent a filtré sur le pseudonyme de type "last_name" de "Brizolier"
      Quand l'agent lit le texte libre "vu Brizolier puis BRIZOLIER"
      Alors la valeur rendue est "vu {pseudonyme} puis {pseudonyme}"

    Scénario: Un nom traduit court n'est remplacé que comme mot entier
      Étant donné que l'agent a filtré sur le pseudonyme de type "last_name" de "Roy"
      Quand l'agent lit le texte libre "vu Roy à Troyes"
      Alors la valeur rendue est "vu {pseudonyme} à Troyes"

    Scénario: Un mot plus long qui contient le nom traduit devient un jeton opaque en entier
      Étant donné que l'agent a filtré sur le pseudonyme de type "last_name" de "Kervadec"
      Quand l'agent lit le texte libre "réf Kervadec2026 pour Kervadec"
      Alors la valeur rendue ne contient plus "Kervadec"
      Et la valeur rendue contient un jeton opaque "value"
      Et la valeur rendue contient "pour {pseudonyme}"

  Règle: Une valeur vide traverse sans changement

    Scénario: Une adresse e-mail vide reste vide
      Quand l'agent obtient le pseudonyme de type "email" de ""
      Alors le pseudonyme est vide
