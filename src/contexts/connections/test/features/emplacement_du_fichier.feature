# language: fr
@connections
Fonctionnalité: Emplacement du fichier de connexions
  Le serveur trouve seul le fichier de connexions du développeur et relit ses modifications à chaque appel, sans redémarrage.
  Une variable d'environnement peut désigner un autre fichier, par exemple celui d'un dépôt d'équipe.

  Règle: Le chemin de SCARIF_CONNECTIONS_FILE prime, puis XDG_CONFIG_HOME s'il est absolu, puis le dossier .config du dossier personnel

    Plan du Scénario: Le fichier lu dépend de l'environnement
      Étant donné la variable SCARIF_CONNECTIONS_FILE "<variable>", la variable XDG_CONFIG_HOME "<xdg>" et le dossier personnel "/home/alice"
      Quand le serveur cherche son fichier de connexions
      Alors le fichier de connexions lu est "<fichier>"

      Exemples:
        | variable                 | xdg          | fichier                                     |
        | /equipe/connections.toml | /xdg         | /equipe/connections.toml                    |
        |                          | /xdg         | /xdg/scarif/connections.toml                |
        |                          | config/local | /home/alice/.config/scarif/connections.toml |
        |                          |              | /home/alice/.config/scarif/connections.toml |

  Règle: L'ancien dossier pg-pseudo n'est lu que tant que le fichier du dossier scarif n'existe pas, et un import écrit toujours dans le dossier scarif

    Plan du Scénario: Le dossier lu suit les fichiers présents
      Étant donné un dossier personnel où existent les fichiers de connexions des dossiers "<présents>"
      Quand le serveur cherche son fichier de connexions
      Alors le fichier de connexions lu est celui du dossier "<lu>"
      Et un import écrirait le fichier de connexions dans le dossier "scarif"

      Exemples:
        | présents          | lu        |
        | scarif, pg-pseudo | scarif    |
        | pg-pseudo         | pg-pseudo |
        | aucun             | scarif    |

    Scénario: Un chemin explicite ignore l'ancien dossier
      Étant donné un dossier personnel où existent les fichiers de connexions des dossiers "pg-pseudo"
      Et la variable SCARIF_CONNECTIONS_FILE qui désigne "/equipe/connections.toml"
      Quand le serveur cherche son fichier de connexions
      Alors le fichier de connexions lu est "/equipe/connections.toml"

  Règle: Un fichier désigné par SCARIF_CONNECTIONS_FILE mais absent est signalé avec son chemin

    Scénario: Sans le fichier désigné ni DBeaver, l'indice cite le chemin désigné et propose de créer le fichier ou de lancer l'import
      Étant donné aucun fichier de connexions
      Quand l'agent liste les connexions
      Alors l'indice cite le chemin du fichier de connexions
      Et l'indice propose de créer le fichier ou de lancer l'import

  Règle: Un fichier qui existe mais ne peut pas être lu rend les connexions DBeaver indisponibles

    Scénario: Un fichier de connexions illisible bloque DBeaver jusqu'à sa correction
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et un fichier de connexions illisible
      Quand l'agent liste les connexions
      Alors l'indice contient "cannot be read"
      Et la connexion "canary-local" est listée comme indisponible
      Et la raison de la connexion "canary-local" contient "connection file"

  Règle: Les modifications du fichier sont prises en compte au prochain appel, sans redémarrer le serveur

    Scénario: Une connexion ajoutée au fichier est listée à l'appel suivant
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app-dev"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Et l'agent a déjà listé les connexions
      Quand le fichier de connexions devient :
        """
        [[connection]]
        name = "app-test"
        host = "db.example.com"
        user = "alice"
        type = "test"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Et l'agent liste les connexions
      Alors la connexion "app-test" est listée comme disponible
      Et la liste ne contient pas la connexion "app-dev"
