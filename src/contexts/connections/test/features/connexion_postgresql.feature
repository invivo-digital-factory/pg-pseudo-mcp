# language: fr
@connections
Fonctionnalité: Déclarer une connexion PostgreSQL dans le fichier
  Chaque bloc [[connection]] du fichier décrit une base PostgreSQL : son nom, son hôte, son utilisateur, son type et la façon d'obtenir son mot de passe.
  Une entrée mal écrite est listée indisponible avec une raison qui nomme le champ fautif, et les autres entrées restent utilisables.

  Règle: Une entrée minimale reçoit le moteur postgresql, le port 5432, la base postgres et le mode TLS prefer

    Scénario: Une entrée sans port, base, mode TLS ni moteur reçoit les valeurs par défaut
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
      Et la connexion "app-prod" est listée avec le moteur "postgresql"
      Et la connexion "app-prod" est listée avec la base "postgres"
      Et la connexion "app-prod" est listée avec le type "prod"
      Et la connexion "app-prod" vise le port 5432 avec le mode TLS "prefer"

    Scénario: Une entrée complète est lue champ par champ
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "billing"
        engine = "postgresql"
        host = "db.example.com"
        port = 6432
        database = "billing"
        user = "alice"
        sslmode = "verify-full"
        type = "test"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "billing" est listée comme disponible
      Et la connexion "billing" est listée avec la base "billing"
      Et la connexion "billing" est listée avec le type "test"
      Et la connexion "billing" vise le port 6432 avec le mode TLS "verify-full"

  Règle: Les champs host, user, type et auth sont obligatoires

    Plan du Scénario: Une entrée sans le champ <champ> est indisponible
      Étant donné le fichier de connexions qui déclare l'entrée suivante sans son champ "<champ>" :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "`<champ>` is required"

      Exemples:
        | champ |
        | host  |
        | user  |
        | type  |
        | auth  |

  Règle: Une valeur invalide ou une clé inconnue rend l'entrée indisponible avec une raison qui la nomme

    Plan du Scénario: La ligne « <ligne> » rend l'entrée indisponible
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <ligne> » remplace celle de même clé :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "<raison>"

      Exemples:
        | ligne                          | raison                                                                 |
        | port = 0                       | `port` must be between 1 and 65535                                     |
        | port = 70000                   | `port` must be between 1 and 65535                                     |
        | port = "5432"                  | `port` must be a whole number                                          |
        | sslmode = "sometimes"          | `sslmode` must be one of disable, allow, prefer, require, verify-ca, verify-full |
        | database = ""                  | `database` must be a non-empty string                                  |
        | type = "staging"               | `type` must be one of dev, test, prod                                  |
        | auth = "kerberos"              | `auth` must be one of pgpass, azure, command                           |
        | ssh_alias = "-oProxyCommand=x" | `ssh_alias` must be a host alias                                       |
        | ssh_alias = "two words"        | `ssh_alias` must be a host alias                                       |
        | colour = "blue"                | unknown key `colour`                                                   |

  Règle: Le moteur est postgresql, par défaut, ou mongodb

    Plan du Scénario: Le moteur <moteur> est refusé en nommant engine
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « engine = <moteur> » remplace celle de même clé :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "<raison>"

      Exemples:
        | moteur  | raison                                      |
        | "mysql" | `engine` must be one of postgresql, mongodb |
        | 3       | `engine` must be a non-empty string         |

  Règle: Les clés propres à MongoDB sont refusées sur une entrée PostgreSQL

    Plan du Scénario: La clé <clé> est refusée sur une entrée PostgreSQL
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <clé> = <valeur> » remplace celle de même clé :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "`<clé>` is only valid when `engine` is"

      Exemples:
        | clé                  | valeur      |
        | srv                  | true        |
        | tls                  | true        |
        | replica_set          | "rs0"       |
        | read_preference      | "secondary" |
        | read_preference_tags | ["a:b"]     |

  Règle: Les seuils de requête lourde se règlent sur une connexion prod seulement

    Scénario: Une connexion prod déclare ses propres seuils
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "shop-prod"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        heavy_cost_limit = 50000
        heavy_scan_limit = 200000
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop-prod" est listée comme disponible

    Plan du Scénario: La ligne « <ligne> » rend l'entrée indisponible
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <ligne> » remplace celle de même clé :
        """
        [[connection]]
        name = "shop"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "pgpass"
        heavy_scan_limit = 200000
        """
      Quand l'agent liste les connexions
      Alors la connexion "shop" est listée comme indisponible
      Et la raison de la connexion "shop" contient "<raison>"

      Exemples:
        | ligne                  | raison                                                 |
        | heavy_cost_limit = 0   | `heavy_cost_limit` must be a positive whole number     |
        | heavy_scan_limit = "1" | `heavy_scan_limit` must be a positive whole number     |
        | type = "test"          | `heavy_scan_limit` is only valid when `type` is "prod" |

  Règle: password_command est exigée par auth = "command" et interdite avec les autres modes

    Scénario: Le mode command sans password_command est refusé
      Étant donné le fichier de connexions qui déclare l'entrée suivante sans son champ "password_command" :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "`password_command` is required when auth is"

    Plan du Scénario: La ligne « <ligne> » est refusée
      Étant donné le fichier de connexions qui déclare l'entrée suivante, où la ligne « <ligne> » remplace celle de même clé :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme indisponible
      Et la raison de la connexion "app" contient "<raison>"

      Exemples:
        | ligne                         | raison                                                              |
        | auth = "pgpass"               | `password_command` is only allowed when auth is                     |
        | auth = "azure"                | `password_command` is only allowed when auth is                     |
        | password_command = []         | `password_command` must be a non-empty list of non-empty strings    |
        | password_command = [""]       | `password_command` must be a non-empty list of non-empty strings    |
        | password_command = [1]        | `password_command` must be a non-empty list of non-empty strings    |
        | password_command = "op read"  | `password_command` must be a list: the program, then its arguments |

  Règle: Chaque entrée a un nom unique

    Scénario: Deux connexions du même nom sont toutes deux indisponibles
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"

        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors toutes les connexions nommées "app" sont indisponibles
      Et la raison de la connexion "app" contient "the name `app` is defined more than once"

    Scénario: Une entrée sans nom est listée sous sa position dans le fichier
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]

        [[connection]]
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "pgpass"
        """
      Quand l'agent liste les connexions
      Alors la connexion "app" est listée comme disponible
      Et la connexion "connection #2" est listée comme indisponible
      Et la raison de la connexion "connection #2" contient "`name` is required"
