# language: fr
@querying
Fonctionnalité: Informations du serveur
  server_info dit à l'agent, sans qu'il ait à deviner, ce que le serveur accepte et garantit : les moteurs, les limites par défaut et les plafonds,
  les types de pseudonymes qu'un filtre peut retraduire, ce qui agit sur les valeurs réelles, la liste autorisée MongoDB et les limites connues.
  Il porte aussi les avertissements de configuration, comme un fichier de politique ignoré.

  Règle: Les limites annoncées sont celles qui s'appliquent

    Scénario: Moteurs, limites par défaut et plafonds
      Quand l'agent demande les informations du serveur
      Alors les moteurs annoncés sont "postgresql" et "mongodb"
      Et les limites par défaut sont 50 lignes, 16384 octets et 10000 ms
      Et les plafonds sont 500 lignes, 65536 octets et 60000 ms
      Et une valeur texte est coupée à 500 caractères

  Règle: Les types de pseudonymes sont annoncés réversibles ou non

    Plan du Scénario: Le type <type> est annoncé <réversibilité>
      Quand l'agent demande les informations du serveur
      Alors le type "<type>" est annoncé <réversibilité>

      Exemples:
        | type        | réversibilité  |
        | email       | réversible     |
        | phone       | réversible     |
        | iban        | réversible     |
        | siren       | réversible     |
        | siret       | réversible     |
        | nir         | réversible     |
        | card_number | réversible     |
        | ip_address  | réversible     |
        | person_name | réversible     |
        | last_name   | réversible     |
        | first_name  | non réversible |
        | address     | non réversible |
        | city        | non réversible |
        | postal_code | non réversible |
        | civility    | non réversible |
        | gender      | non réversible |
        | birth_date  | non réversible |

  Règle: La liste autorisée MongoDB est publiée avec ses plafonds

    Scénario: Les étapes de lecture sont listées, les étapes d'écriture n'y sont pas
      Quand l'agent demande les informations du serveur
      Alors les étapes MongoDB annoncées comptent 26 étapes dont "$match", "$lookup" et "$facet"
      Et les étapes MongoDB annoncées ne contiennent ni "$out", ni "$merge", ni "$function"
      Et "$geoNear" est annoncé seulement en première étape
      Et le plafond annoncé est de 50 étapes, 32 niveaux, 65536 octets et 1000 éléments par tableau

  Règle: Un fichier de politique ignoré est signalé
    Le détail des fichiers de politique est dans la politique de colonnes et la politique de champs MongoDB.

    Scénario: Sans fichier de politique, aucun avertissement
      Quand l'agent demande les informations du serveur
      Alors les informations ne portent aucun avertissement

    Scénario: Un fichier invalide est nommé dans un avertissement
      Étant donné la politique de colonnes partagée suivante :
        """
        [personal]
        "*.supplier_ref" = "numero_fiscal"
        """
      Quand la politique de colonnes est chargée
      Alors un avertissement "column_policy_ignored" nomme le fichier partagé
