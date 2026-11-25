# language: fr
@anonymization
Fonctionnalité: Messages d'erreur et plans de requête
  Une erreur de la base ou un plan d'exécution peut citer une valeur réelle : un message d'erreur sur une donnée
  invalide, une clé en double, un filtre recopié dans le plan. Ces textes passent par les mêmes détecteurs que les
  résultats, et toute valeur citée entre guillemets dans une erreur de donnée devient un jeton opaque, sauf un pseudonyme.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Une erreur de donnée de PostgreSQL ne cite jamais la valeur fautive

    Scénario: Une valeur invalide citée dans le message devient un jeton opaque
      Quand la base répond l'erreur "22P02" "invalid input syntax for type integer: \"Brizolier\""
      Alors le message rendu ne contient plus "Brizolier"
      Et le message rendu commence par "invalid input syntax for type integer: \"<value id="

    Scénario: La clé en double du détail d'une violation d'unicité devient un jeton opaque
      Quand la base répond l'erreur "23505" "duplicate key value violates unique constraint" avec le détail "Key (nom)=(Brizolier) already exists."
      Alors le détail rendu ne contient plus "Brizolier"
      Et le détail rendu commence par "Key (nom)=(<value id="
      Et le détail rendu se termine par ") already exists."

    Scénario: Un pseudonyme cité dans une erreur de donnée reste lisible
      Quand l'agent obtient le pseudonyme de type "email" de "odrigan.hazvekmor@canary-mail.fr"
      Et la base répond l'erreur "23505" "duplicate key value violates unique constraint" avec le détail "Key (email)=({pseudonyme}) already exists."
      Alors le détail rendu est "Key (email)=({pseudonyme}) already exists."

    Scénario: Le contexte d'exécution d'une erreur n'est jamais transmis
      Quand la base répond l'erreur "22P02" "invalid input syntax for type integer: \"Brizolier\""
      Alors le message rendu n'a pas de contexte d'exécution

    Scénario: Une erreur de structure garde les noms d'objets cités
      Quand la base répond l'erreur "42P01" "relation \"customers\" does not exist"
      Alors le message rendu est "relation \"customers\" does not exist"

    Scénario: Une adresse e-mail dans une erreur de structure est quand même remplacée
      Quand la base répond l'erreur "42703" "column ylwenna.kervadec@canary-mail.fr does not exist"
      Alors le message rendu ne contient plus "ylwenna.kervadec@canary-mail.fr"

  Règle: Un message du serveur MongoDB masque les valeurs citées, sauf les opérateurs connus et les pseudonymes

    Scénario: La valeur que le serveur n'a pas su convertir est masquée, l'opérateur reste lisible
      Étant donné les noms connus du serveur "$toInt, $convert"
      Quand le serveur MongoDB répond "Failed to parse number 'Kervadec' in $convert with no onError value: '$toInt'"
      Alors le message rendu ne contient plus "Kervadec"
      Et le message rendu contient "'$toInt'"
      Et le message rendu commence par "Failed to parse number '<value id="

    Plan du Scénario: Une valeur citée qui contient une apostrophe ou un guillemet est masquée en entier
      Étant donné les noms connus du serveur "$toInt, $convert"
      Quand le serveur MongoDB répond "<message>"
      Alors le message rendu ne contient plus "Zivrelle"

      Exemples:
        | message                                                     |
        | Failed to parse number 'O'Zivrelle' in $convert             |
        | Failed to parse number 'O' Zivrelle' in $convert            |
        | Failed to parse number 'Zivrelle in $convert with no value  |

  Règle: Un plan d'exécution garde sa structure et perd ses valeurs réelles

    Scénario: Le filtre recopié dans un plan d'exécution est pseudonymisé
      Quand l'agent lit le plan suivant :
        """
        {"Plan": {"Node Type": "Seq Scan", "Relation Name": "customers", "Filter": "(email = 'ylwenna.kervadec@canary-mail.fr'::text)"}}
        """
      Alors le plan rendu ne contient plus "ylwenna.kervadec@canary-mail.fr"
      Et le plan rendu garde "Node Type" à "Seq Scan"
      Et le plan rendu garde "Relation Name" à "customers"
