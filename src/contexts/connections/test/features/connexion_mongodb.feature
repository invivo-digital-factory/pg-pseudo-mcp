# language: fr
@connections
Fonctionnalité: Déclarer une connexion MongoDB dans le fichier
  Une entrée engine = "mongodb" atteint un replica set, par exemple un cluster Atlas, lu par les outils find et aggregate.
  Son mot de passe vient toujours d'une commande, et lister les connexions n'exécute jamais cette commande ni n'ouvre de tunnel.

  Règle: Par défaut, l'hôte est un nom SRV résolu avec TLS, sans port

    Scénario: Une entrée minimale passe par la résolution SRV avec TLS
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-dev" est listée comme disponible
      Et la connexion "orders-dev" est listée avec le moteur "mongodb"
      Et la connexion "orders-dev" est listée sans base par défaut
      Et la connexion MongoDB "orders-dev" résout le nom SRV "cluster0.example.com" avec TLS

  Règle: Avec srv = false, l'hôte est un membre joint sur le port 27017 avec TLS, sauf réglage contraire

    Scénario: Une entrée sans SRV vise le port 27017 avec TLS
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "mongo-0.example.com"
        srv = false
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-dev" est listée comme disponible
      Et la connexion MongoDB "orders-dev" vise le membre "mongo-0.example.com" sur le port 27017 avec TLS

    Scénario: Une entrée sans SRV avec port, jeu de réplicas, base et TLS désactivé est lue champ par champ
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-local"
        engine = "mongodb"
        host = "mongo-0.example.com"
        port = 1024
        srv = false
        tls = false
        replica_set = "rs0"
        database = "shop"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-local" est listée comme disponible
      Et la connexion "orders-local" est listée avec le moteur "mongodb"
      Et la connexion "orders-local" est listée avec la base "shop"
      Et la connexion MongoDB "orders-local" vise le membre "mongo-0.example.com" sur le port 1024 sans TLS
      Et la connexion MongoDB "orders-local" déclare le jeu de réplicas "rs0"

  Règle: La préférence de lecture choisit le membre du replica set qui sert chaque lecture
    Sans read_preference, chaque lecture va de préférence à un secondaire, et au primaire s'il n'y en a aucun.
    read_preference_tags restreint les lectures aux membres qui portent toutes ces étiquettes, avec un mode autre que primary.
    Les secondaires sont joints comme le primaire, par le même front SOCKS5 et le même bastion.

    Scénario: Sans read_preference, la lecture préfère un secondaire
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-prod"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-prod" est listée comme disponible
      Et la connexion MongoDB "orders-prod" lit avec la préférence "secondaryPreferred" sans étiquette

    Scénario: Les lectures vont aux secondaires étiquetés pour l'analyse
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-prod"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        read_preference = "secondary"
        read_preference_tags = ["nodeType:ANALYTICS"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-prod" est listée comme disponible
      Et la connexion MongoDB "orders-prod" lit avec la préférence "secondary" et l'étiquette "nodeType" = "ANALYTICS"

    Scénario: Le primaire peut être imposé
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-prod"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        read_preference = "primary"
        """
      Quand l'agent liste les connexions
      Alors la connexion MongoDB "orders-prod" lit avec la préférence "primary" sans étiquette

  Règle: Une clé incompatible avec MongoDB ou avec la résolution SRV rend l'entrée indisponible en la nommant

    Plan du Scénario: La ligne « <ligne> » est refusée sur une entrée MongoDB
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <ligne> » remplace celle de même clé :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-dev" est listée comme indisponible
      Et la raison de la connexion "orders-dev" contient "<raison>"

      Exemples: Clés incompatibles avec la résolution SRV
        | ligne       | raison                                                 |
        | port = 1024 | `port` is not valid when `srv` is true; use `srv = false` |
        | tls = false | `tls` must be true while `srv` is true                 |

      Exemples: Clés propres à PostgreSQL et valeurs mal typées
        | ligne               | raison                                                  |
        | sslmode = "require" | `sslmode` is not valid when `engine` is "mongodb"; use `tls` |
        | srv = "yes"         | `srv` must be true or false                             |
        | tls = 1             | `tls` must be true or false                             |
        | replica_set = ""    | `replica_set` must be a non-empty string                |
        | replica_set = 3     | `replica_set` must be a non-empty string                |
        | database = ""       | `database` must be a non-empty string                   |

      Exemples: Préférence de lecture invalide
        | ligne                                         | raison                                                                 |
        | read_preference = "Secondary"                 | `read_preference` must be one of primary, primaryPreferred, secondary  |
        | read_preference_tags = ["nodeType:ANALYTICS"] | `read_preference_tags` needs a `read_preference` other than primary    |
        | read_preference_tags = ["nodeType"]          | `read_preference_tags` must be a non-empty list of "key:value" strings |

      @ae11
      Exemples: Modes d'authentification autres que command
        | ligne             | raison                                      |
        | auth = "azure"    | `auth` must be "command" when `engine` is "mongodb" |
        | auth = "pgpass"   | `auth` must be "command" when `engine` is "mongodb" |
        | auth = "kerberos" | `auth` must be "command" when `engine` is "mongodb" |

  Règle: Lister une connexion MongoDB n'exécute pas sa commande de mot de passe

    Scénario: Une connexion MongoDB en mode commande est listée disponible sans exécuter la commande
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-dev" est listée comme disponible
      Et la commande de mot de passe n'a pas été exécutée
      Et la liste ne montre ni l'hôte "cluster0.example.com", ni l'utilisateur "alice", ni le secret "marqueur"

    Scénario: Une connexion MongoDB derrière un alias SSH est listée sans ouvrir le tunnel ni exécuter la commande
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          Port 2222
          User jump
        """
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        ssh_alias = "bastion"
        """
      Quand l'agent liste les connexions
      Alors la connexion "orders-dev" est listée comme disponible
      Et la connexion MongoDB "orders-dev" passe par le bastion "bastion.example.com:2222" en tant que "jump"
      Et la commande de mot de passe n'a pas été exécutée
      Et la liste ne montre ni l'hôte "bastion.example.com", ni l'utilisateur "jump", ni le secret "marqueur"

  Règle: Chaque moteur a ses outils : les outils SQL renvoient vers find ou aggregate, les outils de documents vers query

    Scénario: Une requête SQL sur une connexion MongoDB renvoie vers find ou aggregate sans exécuter la commande
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        """
      Quand l'agent exécute une requête de lecture sur la connexion "orders-dev"
      Alors la requête échoue avec une erreur qui contient "this connection is of engine mongodb; use the find or aggregate tool"
      Et l'erreur de la requête ne contient pas "cluster0.example.com"
      Et la commande de mot de passe n'a pas été exécutée

    Scénario: La lecture du catalogue SQL d'une connexion MongoDB renvoie vers find ou aggregate sans exécuter la commande
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "orders-dev"
        engine = "mongodb"
        host = "cluster0.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        """
      Quand l'agent lit le catalogue de la connexion "orders-dev"
      Alors la requête échoue avec une erreur qui contient "find or aggregate"
      Et la commande de mot de passe n'a pas été exécutée

    Scénario: Une lecture de documents sur une connexion PostgreSQL renvoie vers query sans exécuter la commande
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        """
      Quand l'agent lit les documents de la collection "orders" sur la connexion "app"
      Alors la requête échoue avec une erreur qui contient "this connection is not of engine mongodb; use the query tool"
      Et la commande de mot de passe n'a pas été exécutée
