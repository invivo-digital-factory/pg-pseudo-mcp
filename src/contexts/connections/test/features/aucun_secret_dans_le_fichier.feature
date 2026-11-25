# language: fr
@connections
Fonctionnalité: Aucun secret dans le fichier de connexions
  Le fichier de connexions peut vivre dans un dépôt d'équipe : il ne porte ni mot de passe, ni jeton, ni phrase secrète.
  Une clé qui ressemble à un secret est refusée, et sa valeur n'est jamais recopiée dans une raison ni dans un indice.

  Règle: Une clé d'entrée qui ressemble à un secret rend l'entrée indisponible sans recopier sa valeur

    Plan du Scénario: La clé <clé> est refusée comme un secret
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <clé> = "zq7x-valeur" » remplace celle de même clé :
        """
        [[connection]]
        name = "app-prod"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-prod" est listée comme indisponible
      Et la raison de la connexion "app-prod" contient "the connection file must not hold secrets; remove `<clé>`"
      Et la liste ne montre ni l'hôte "db.example.com", ni l'utilisateur "alice", ni le secret "zq7x-valeur"

      Exemples:
        | clé           |
        | password      |
        | passwd        |
        | pgpassword    |
        | token         |
        | access_token  |
        | passphrase    |
        | secret        |
        | db_password   |
        | Password      |

  Règle: password_command n'est pas un secret : elle désigne le programme qui donne le mot de passe

    Scénario: Une entrée en mode command est disponible
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app-prod"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-prod" est listée comme disponible

  Règle: Une clé de premier niveau qui ressemble à un secret bloque DBeaver sans être recopiée

    Plan du Scénario: La clé de premier niveau <clé> rend les connexions DBeaver indisponibles
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et le fichier de connexions qui contient seulement la ligne « <clé> = "zq7x-top-level" »
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme indisponible
      Et la raison de la connexion "canary-local" contient "connection file"
      Et l'indice contient "must not hold secrets; remove the top-level key `<clé>`"
      Et l'indice ne contient pas "zq7x-top-level"

      Exemples:
        | clé       |
        | password  |
        | api_token |

  Règle: Une erreur de syntaxe ne recopie jamais le contenu du fichier

    Scénario: Une erreur de syntaxe sur une ligne qui porte un mot de passe ne le recopie pas dans l'indice
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "a"
        password = "zq7x"k"
        """
      Quand l'agent liste les connexions
      Alors l'indice cite la ligne 3
      Et l'indice ne contient pas "zq7x"
