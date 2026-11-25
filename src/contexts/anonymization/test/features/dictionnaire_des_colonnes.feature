# language: fr
@anonymization
Fonctionnalité: Dictionnaire des noms de colonnes et de champs
  Le nom d'une colonne ou d'un champ suffit souvent à dire ce qu'il contient : un dictionnaire de noms personnels,
  en français et en anglais, associe un type à chaque nom reconnu. Une colonne reconnue voit sa valeur entière
  remplacée, sans dépendre de la forme de la valeur. Un nom technique n'est pas un indice : ses valeurs passent alors
  par les détecteurs, comme toute colonne au nom inconnu.

  Règle: Un nom de colonne personnel désigne le type de son contenu, quelle que soit son écriture

    Plan du Scénario: La colonne "<colonne>" contient des valeurs de type <type>
      Quand le dictionnaire lit le nom de colonne "<colonne>"
      Alors le dictionnaire y reconnaît le type "<type>"

      Exemples: les noms simples
        | colonne        | type        |
        | email          | email       |
        | courriel       | email       |
        | telephone      | phone       |
        | iban           | iban        |
        | siren          | siren       |
        | siret          | siret       |
        | nir            | nir         |
        | carte_bancaire | card_number |
        | adresse_ip     | ip_address  |
        | nom            | last_name   |
        | prenom         | first_name  |
        | adresse        | address     |
        | ville          | city        |
        | code_postal    | postal_code |
        | civilite       | civility    |
        | genre          | gender      |
        | date_naissance | birth_date  |

      Exemples: les écritures en camelCase, kebab-case, majuscules, accents et suffixes
        | colonne            | type       |
        | telephonePortable  | phone      |
        | telephone_secours  | phone      |
        | customerEmail      | email      |
        | IPAddress          | ip_address |
        | Prénom             | first_name |
        | date-de_naissance2 | birth_date |

  Règle: La correspondance la plus longue l'emporte

    Plan du Scénario: La colonne "<colonne>" est lue comme <type>, pas comme le mot plus court qu'elle contient
      Quand le dictionnaire lit le nom de colonne "<colonne>"
      Alors le dictionnaire y reconnaît le type "<type>"

      Exemples:
        | colonne       | type       |
        | email_address | email      |
        | nom_famille   | last_name  |
        | adresse_ip    | ip_address |

  Règle: Un nom de colonne technique n'est pas un indice de donnée personnelle, même s'il contient un mot personnel

    Plan du Scénario: Le nom de colonne "<colonne>" ne désigne aucun type
      Quand le dictionnaire lit le nom de colonne "<colonne>"
      Alors le dictionnaire n'y reconnaît aucun type

      Exemples: des noms sans mot personnel, même quand ils commencent comme un mot personnel
        | colonne      |
        | id           |
        | customer_id  |
        | status       |
        | created_at   |
        | amount       |
        | titulaire    |
        | nombre       |
        | connaissance |
        | capacity     |

      Exemples: des expressions techniques qui contiennent un mot personnel
        | colonne         |
        | adresse_mac     |
        | mac_address     |
        | nom_utilisateur |
        | client_id       |

  Règle: Une colonne reconnue par le dictionnaire voit sa valeur entière remplacée

    Scénario: La colonne "nom" remplace même une valeur qui n'a pas la forme d'un nom
      Étant donné un répertoire de données neuf
      Quand l'agent lit la valeur "lot-42 / colis" dans la colonne "nom"
      Alors la valeur est pseudonymisée
      Et la valeur rendue ne contient plus "colis"
      Et la colonne est signalée "last_name" pour la raison "dictionary"
