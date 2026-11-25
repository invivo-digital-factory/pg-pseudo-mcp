# language: fr
@connections
Fonctionnalité: Fichier de connexions invalide
  Un fichier mal formé est signalé dans l'indice de list_connections, avec sa ligne et sa colonne pour une erreur de syntaxe, jamais avec son contenu.
  Tant qu'il n'est pas corrigé, les connexions DBeaver restent listées mais indisponibles : un fichier cassé ne rouvre pas en silence ce qu'il masquait.

  Règle: Une erreur de syntaxe est signalée avec sa position et bloque DBeaver

    @ae2
    Scénario: Un fichier avec une erreur de syntaxe est signalé avec sa ligne et rend les connexions DBeaver indisponibles
      Étant donné un espace de travail DBeaver avec les connexions "dbeaver-a" et "dbeaver-b" dont les mots de passe sont enregistrés
      Et le fichier de connexions suivant :
        """
        # connexions de l'équipe
        #
        #
        #
        #
        #
        #
        #
        #
        #
        [[connection]]
        name = "a" "b"
        """
      Quand l'agent liste les connexions
      Alors l'indice cite la ligne 12
      Et l'indice contient "column 12"
      Et l'indice contient "fix its syntax"
      Et la connexion "dbeaver-a" est listée comme indisponible
      Et la connexion "dbeaver-b" est listée comme indisponible
      Et la raison de la connexion "dbeaver-a" contient "connection file"

    Scénario: Un fichier qui porte déjà son indice n'y ajoute pas la proposition de le créer
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "a" "b"
        """
      Quand l'agent liste les connexions
      Alors l'indice cite la ligne 2
      Et l'indice ne contient pas "run `scarif-database-mcp connections import-dbeaver`"

  Règle: Un réglage de premier niveau invalide ou inconnu est signalé et bloque DBeaver

    Plan du Scénario: La ligne « <ligne> » bloque DBeaver
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions qui contient seulement la ligne « <ligne> »
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme indisponible
      Et la raison de la connexion "canary-local" contient "DBeaver connections stay unavailable until it is fixed"
      Et l'indice contient "<indice>"

      Exemples:
        | ligne                 | indice                                    |
        | read_dbeaver = "no"   | `read_dbeaver` must be true or false      |
        | read_dbeaver = 0      | `read_dbeaver` must be true or false      |
        | colour = "blue"       | unknown top-level key `colour`            |

  Règle: Les connexions se déclarent en tableau de tables [[connection]]

    Plan du Scénario: La ligne « <ligne> » est signalée dans l'indice
      Étant donné le fichier de connexions qui contient seulement la ligne « <ligne> »
      Quand l'agent liste les connexions
      Alors l'indice contient "<indice>"

      Exemples:
        | ligne              | indice                                                         |
        | connection = "app" | `connection` must be an array of tables, written [[connection]] |
        | connection = [1]   | each `connection` must be a table                              |
