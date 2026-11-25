# language: fr
@anonymization
Fonctionnalité: Un pseudonyme réaliste pour chaque type de valeur personnelle
  Chaque type de valeur personnelle a son générateur : le pseudonyme a l'allure d'une vraie valeur du même type et
  passe les mêmes contrôles (clé de Luhn, modulo 97, clé du NIR), pour qu'une requête de l'agent reste plausible.
  Les e-mails, téléphones, IBAN, cartes et adresses IP sont tirés dans des espaces réservés à la documentation ou fictifs ;
  un SIREN, un SIRET ou un NIR pseudonyme est seulement un numéro valide tiré au hasard.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Les identifiants reçoivent un pseudonyme valide, tiré dans un espace réservé quand il en existe un

    Plan du Scénario: Une valeur de type <type> reçoit un pseudonyme qui <propriété>
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme diffère de la valeur
      Et le pseudonyme <propriété>

      Exemples:
        | type        | valeur                            | propriété                                                     |
        | email       | ylwenna.kervadec@canary-mail.fr   | est une adresse sur example.com, example.net ou example.org   |
        | phone       | 06 41 27 83 95                    | est un numéro des blocs fictifs, présenté comme l'original    |
        | phone       | +33 6 41 27 83 95                 | est un numéro des blocs fictifs, présenté comme l'original    |
        | iban        | FR76 3000 4000 0312 3456 7890 143 | est un IBAN valide de la banque fictive 99999                 |
        | siren       | 418273652                         | a 9 chiffres et une clé de Luhn valide                        |
        | siret       | 41827365210001                    | a 14 chiffres, une clé de Luhn valide et un SIREN valide      |
        | nir         | 2851175123456 22                  | est un NIR à clé valide qui garde le sexe de l'original       |
        | card_number | 4970 1012 3456 7810               | commence par 400000 et passe le contrôle de Luhn              |
        | ip_address  | 192.168.10.4                      | est une adresse des plages IPv4 de documentation              |
        | ip_address  | fd12:3456:789a::1                 | est une adresse du préfixe IPv6 de documentation 2001:db8     |

  Règle: Les noms et les lieux reçoivent un nom ou un lieu plausible

    Plan du Scénario: Une valeur de type <type> reçoit un pseudonyme qui <propriété>
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme diffère de la valeur
      Et le pseudonyme <propriété>

      Exemples:
        | type        | valeur           | propriété                                           |
        | person_name | Ylwenna Kervadec | est un prénom du catalogue suivi d'un nom de famille |
        | last_name   | Kervadec         | est un seul nom de famille                          |
        | first_name  | Ylwenna          | est un prénom du catalogue                          |
        | address     | 212 rue des Lilas | commence par un numéro suivi du même type de voie  |
        | postal_code | 20200            | a 5 chiffres                                        |

  Règle: La civilité et le genre restent dans le vocabulaire de l'original

    Plan du Scénario: La valeur "<valeur>" de type <type> devient l'une des valeurs "<vocabulaire>"
      Quand l'agent obtient le pseudonyme de type "<type>" de "<valeur>"
      Alors le pseudonyme est l'une des valeurs "<vocabulaire>"

      Exemples:
        | type     | valeur   | vocabulaire       |
        | civility | Madame   | Monsieur, Madame  |
        | civility | Mme      | M., Mme           |
        | civility | MME      | M., MME           |
        | civility | Mr       | Mr, Mrs           |
        | gender   | F        | M, F              |
        | gender   | H        | H, F              |
        | gender   | homme    | homme, femme      |
        | gender   | Féminin  | Masculin, Féminin |
        | gender   | 2        | 1, 2              |

  Règle: Quand l'espace des pseudonymes est épuisé, un jeton opaque remplace la valeur

    Scénario: Quand aucun numéro fictif n'est libre, le téléphone reçoit un jeton opaque stable et non traduisible
      Étant donné que l'espace des téléphones fictifs est limité à 1 numéro
      Quand l'agent obtient le pseudonyme de type "phone" de "06 41 27 83 95"
      Et l'agent obtient le pseudonyme de type "phone" de "06 52 18 74 30"
      Et l'agent obtient le pseudonyme de type "phone" de "06 52 18 74 30"
      Alors le pseudonyme 1 est un numéro des blocs fictifs
      Et le pseudonyme 2 est un jeton opaque "phone"
      Et le pseudonyme 3 est identique au pseudonyme 2
      Et le pseudonyme 2 ne se traduit pas en retour
