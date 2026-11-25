# language: fr
@anonymization
Fonctionnalité: Casse et forme des pseudonymes
  Un pseudonyme reprend la présentation de la valeur qu'il remplace : la casse de chaque mot d'un nom, les séparateurs
  d'un numéro, le format d'une date. L'agent lit ainsi des données qui ont l'air des vraies, sans qu'un détail
  de présentation ne trahisse la valeur réelle, et sans qu'aucun mot entourant la valeur ne soit recopié.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Un nom garde la casse de l'original, mot par mot ; un nom de personne devient toujours un prénom et un nom

    Plan du Scénario: "<valeur>" de type <type> devient <forme>
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme diffère de la valeur
      Et le pseudonyme est <forme>

      Exemples:
        | type        | valeur            | forme                                          |
        | person_name | Ylwenna Kervadec  | en deux mots en casse titre                    |
        | person_name | ODRIGAN HAZVEKMOR | en deux mots en majuscules                     |
        | person_name | maëlouz brizolier | en deux mots en minuscules                     |
        | person_name | Ylwenna KERVADEC  | un prénom en casse titre et un nom en majuscules |
        | last_name   | KERVADEC          | en un mot en majuscules                        |
        | last_name   | Kervadec          | en un mot en casse titre                       |
        | first_name  | ylwenna           | en minuscules                                  |
        | city        | PLOUZANÉ          | en majuscules                                  |

  Règle: Un prénom composé en casse titre reçoit un prénom en casse titre, jamais en majuscules

    Plan du Scénario: Le prénom composé "<valeur>" reçoit un prénom en casse titre
      Quand l'agent obtient le pseudonyme de type "first_name" de "<valeur>"
      Alors le pseudonyme est en casse titre

      Exemples:
        | valeur          |
        | Ylwenna-Maëlouz |
        | Ylwenna Maëlouz |

  Règle: Un numéro garde ses séparateurs et ses groupes

    Plan du Scénario: Le <type> "<valeur>" garde la présentation <présentation>
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme a la présentation "<présentation>"

      Exemples:
        | type        | valeur                            | présentation                      |
        | phone       | 06 41 27 83 95                    | xx xx xx xx xx                    |
        | phone       | 07.58.14.92.36                    | xx.xx.xx.xx.xx                    |
        | phone       | 0652187430                        | xxxxxxxxxx                        |
        | phone       | +33 6 41 27 83 95                 | +xx x xx xx xx xx                 |
        | phone       | 0033 6 41 27 83 95                | xxxx x xx xx xx xx                |
        | phone       | 33641278395                       | xxxxxxxxxxx                       |
        | iban        | FR76 3000 4000 0312 3456 7890 143 | xxxx xxxx xxxx xxxx xxxx xxxx xxx |
        | iban        | FR7630004000031234567890143       | xxxxxxxxxxxxxxxxxxxxxxxxxxx       |
        | siren       | 418 273 652                       | xxx xxx xxx                       |
        | nir         | 2851175123456 22                  | xxxxxxxxxxxxx xx                  |
        | card_number | 4970 1012 3456 7810               | xxxx xxxx xxxx xxxx               |

  Règle: Un mobile reste un mobile, un fixe reste un fixe

    Plan du Scénario: Le numéro <nature> "<valeur>" reçoit un numéro <nature> fictif commençant par <début>
      Quand l'agent obtient le pseudonyme de type "phone" de "<valeur>"
      Alors le pseudonyme commence par l'un de "<début>"

      Exemples:
        | nature | valeur         | début                  |
        | mobile | 06 41 27 83 95 | 06 39 98, 07 39 98     |
        | fixe   | 01 45 67 89 10 | 01, 02, 03, 04, 05     |

  Règle: Le texte qui entoure un numéro, une date ou une adresse IP n'est jamais recopié dans le pseudonyme

    Plan du Scénario: Le <type> "<valeur>" ne laisse passer aucun mot de l'original
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme ne contient aucun mot de l'original
      Et le pseudonyme a la présentation "<présentation>"

      Exemples:
        | type       | valeur                                     | présentation                      |
        | phone      | 06 41 27 83 95 (Ylwenna Kervadec)          | xx xx xx xx xx                    |
        | phone      | tel: 0641278395                            | xx xx xx xx xx                    |
        | iban       | FR76 3000 4000 0312 3456 7890 143 Kervadec | xxxx xxxx xxxx xxxx xxxx xxxx xxx |
        | siren      | 418273652 Kervadec                         | xxxxxxxxx                         |
        | birth_date | 1985-11-02 Ylwenna                         | xxxx-xx-xx                        |

  Règle: Seul un suffixe technique valide est gardé : l'heure et le fuseau d'une date, la longueur de préfixe d'une adresse IP

    Plan du Scénario: Le <type> "<valeur>" garde le suffixe "<suffixe>"
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme diffère de la valeur
      Et le pseudonyme se termine par "<suffixe>"

      Exemples:
        | type       | valeur               | suffixe    |
        | birth_date | 1985-11-02T10:30:00Z | T10:30:00Z |
        | birth_date | 1985-03-12 00:00:00  | 00:00:00   |
        | ip_address | 192.168.10.4/24      | /24        |

    Scénario: Un texte collé derrière une adresse IP n'est pas pris pour une longueur de préfixe
      Quand l'agent obtient le pseudonyme de type "ip_address" de "192.168.10.4/Ylwenna"
      Alors le pseudonyme ne contient aucun mot de l'original
      Et le pseudonyme est une adresse des plages IPv4 de documentation
