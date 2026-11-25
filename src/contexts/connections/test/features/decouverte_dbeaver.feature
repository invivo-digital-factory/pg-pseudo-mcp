# language: fr
@connections
Fonctionnalité: Découverte des connexions DBeaver
  En repli du fichier de connexions, le serveur lit les connexions PostgreSQL déjà configurées dans l'espace de travail DBeaver du développeur.
  Il déchiffre les identifiants que DBeaver a enregistrés pour s'y connecter, mais n'en montre jamais l'hôte, l'utilisateur, le mot de passe ni le tunnel.

  Règle: Une connexion n'est disponible que si DBeaver en a enregistré l'utilisateur et le mot de passe

    Scénario: Une connexion dont le mot de passe est enregistré est listée comme utilisable
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme disponible
      Et la connexion "canary-local" est listée avec la source "dbeaver"
      Et la liste ne montre ni l'hôte "db.internal.example", ni l'utilisateur "reader", ni le secret "reader-secret"

    Scénario: Une connexion sans mot de passe enregistré est listée avec la raison
      Étant donné un espace de travail DBeaver où la connexion "canary-local" n'a pas de mot de passe enregistré
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme indisponible
      Et la raison indique qu'aucun mot de passe n'est enregistré dans DBeaver

    Scénario: Un mot de passe marqué enregistré mais absent du fichier d'identifiants évoque le mot de passe maître de DBeaver
      Étant donné un espace de travail DBeaver dont le fichier data-sources.json est :
        """
        {"connections": {"pg-1": {"provider": "postgresql", "name": "app-dev", "save-password": true,
          "configuration": {"host": "db.example.com", "port": "5432", "database": "shop", "type": "dev"}}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-dev" est listée comme indisponible
      Et la raison de la connexion "app-dev" contient "credentials protected by DBeaver master password or secure storage"

    Scénario: L'utilisateur et le mot de passe écrits dans la configuration priment sur le fichier d'identifiants
      Étant donné un espace de travail DBeaver dont le fichier data-sources.json est :
        """
        {"connections": {"pg-1": {"provider": "postgresql", "name": "app-dev", "save-password": true,
          "configuration": {"host": "db.example.com", "database": "shop", "type": "dev",
                            "user": "alice", "password": "en-clair"}}}}
        """
      Et dont les identifiants enregistrés sont :
        """
        {"pg-1": {"#connection": {"user": "bob", "password": "chiffre"}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-dev" est listée comme disponible
      Et la connexion "app-dev" se connecte en tant que "alice" avec le mot de passe "en-clair"

  Règle: Tous les projets et tous les fichiers data-sources de l'espace de travail sont lus

    Scénario: Un fichier data-sources secondaire est lu avec celui par défaut
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et un second fichier data-sources-2.json qui déclare la connexion "app-dev"
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme disponible
      Et la connexion "app-dev" est listée avec la source "dbeaver"

    Scénario: Un fichier data-sources illisible est signalé sans empêcher de lire les autres projets
      Étant donné un espace de travail DBeaver où la connexion "canary-local" a son mot de passe enregistré
      Et un projet DBeaver "Chiffre" dont le fichier data-sources.json n'est pas du JSON
      Quand l'agent liste les connexions
      Alors la connexion "canary-local" est listée comme disponible
      Et la connexion "Chiffre/data-sources.json" est listée comme indisponible
      Et la raison de la connexion "Chiffre/data-sources.json" contient "data-sources file is not valid JSON"

  Règle: Seules les connexions PostgreSQL à authentification native ou pgpass sont prises en charge

    Plan du Scénario: Une connexion <fournisseur> à authentification <modèle> est listée non prise en charge
      Étant donné un espace de travail DBeaver avec une connexion "legacy" du fournisseur "<fournisseur>" au modèle d'authentification "<modèle>"
      Quand l'agent liste les connexions
      Alors la connexion "legacy" est listée comme non prise en charge
      Et la raison de la connexion "legacy" contient "<raison>"

      Exemples:
        | fournisseur | modèle          | raison                                                         |
        | mysql       | native          | engine mysql is not supported, only PostgreSQL                 |
        | postgresql  | postgres_kerberos | authentication model postgres_kerberos is not supported in this version |

  Règle: L'hôte, le port, la base et le mode TLS viennent des champs, d'une URL JDBC ou de variables DBeaver

    Scénario: Une connexion en mode URL prend l'hôte, le port, la base et le mode TLS de l'URL
      Étant donné un espace de travail DBeaver dont le fichier data-sources.json est :
        """
        {"connections": {"pg-1": {"provider": "postgresql", "name": "app-dev", "save-password": true,
          "configuration": {"configurationType": "URL", "url": "jdbc:postgresql://db.example.com:6543/shop?sslmode=require",
                            "type": "dev", "user": "alice", "password": "s3cret"}}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-dev" est listée avec la base "shop"
      Et la connexion "app-dev" vise l'hôte "db.example.com"
      Et la connexion "app-dev" vise le port 6543 avec le mode TLS "require"

    Plan du Scénario: Une configuration « <défaut> » rend la connexion indisponible
      Étant donné un espace de travail DBeaver dont le fichier data-sources.json est :
        """
        {"connections": {"pg-1": {"provider": "postgresql", "name": "app-dev", "save-password": true,
          "configuration": {<configuration>, "type": "dev", "user": "alice", "password": "s3cret"}}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-dev" est listée comme indisponible
      Et la raison de la connexion "app-dev" contient "<raison>"

      Exemples:
        | défaut                       | configuration                                                                                   | raison                                      |
        | URL à plusieurs hôtes        | "configurationType": "URL", "url": "jdbc:postgresql://db1.example.com,db2.example.com/shop"     | connection url must name exactly one host   |
        | mode TLS inconnu             | "host": "db.example.com", "provider-properties": {"sslmode": "sometimes"}                       | unsupported sslmode 'sometimes'             |
        | variable sans valeur         | "host": "${SCARIF_TEST_UNSET_VARIABLE}"                                                         | unresolved variable ${SCARIF_TEST_UNSET_VARIABLE} |
        | port non numérique           | "host": "db.example.com", "port": "five"                                                        | connection port is not a valid number       |

    Scénario: Le mode TLS des propriétés du pilote et la valeur par défaut d'une variable sont appliqués
      Étant donné un espace de travail DBeaver dont le fichier data-sources.json est :
        """
        {"connections": {"pg-1": {"provider": "postgresql", "name": "app-dev", "save-password": true,
          "configuration": {"host": "${SCARIF_TEST_UNSET_VARIABLE:db.example.com}", "type": "dev",
                            "provider-properties": {"sslmode": "verify-ca"}, "user": "alice", "password": "s3cret"}}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-dev" est listée comme disponible
      Et la connexion "app-dev" vise l'hôte "db.example.com"
      Et la connexion "app-dev" vise le port 5432 avec le mode TLS "verify-ca"

  Règle: Deux connexions DBeaver du même nom reçoivent chacune un suffixe stable

    Scénario: Les doublons de nom sont listés sous deux noms suffixés qui ne changent pas d'un appel à l'autre
      Étant donné un espace de travail DBeaver avec deux connexions nommées "app"
      Quand l'agent liste les connexions
      Alors deux connexions sont listées sous le nom "app" suivi d'un suffixe distinct
      Et une nouvelle liste donne les mêmes noms suffixés

  Règle: Un tunnel SSH DBeaver n'est utilisé que si son authentification est complète et prise en charge

    Plan du Scénario: Un tunnel en <authentification> avec ce qui est enregistré est utilisable
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" passe par un tunnel SSH en "<authentification>" avec la clé "<clé>", l'utilisateur SSH "<utilisateur>" et le mot de passe SSH "<mot de passe>"
      Quand l'agent liste les connexions
      Alors la connexion "tunnel-dev" est listée comme disponible

      Exemples:
        | authentification | clé           | utilisateur | mot de passe |
        | PUBLIC_KEY       | {clé de test} | jump        |              |
        | PASSWORD         |               | jump        | s3cret       |
        | AGENT            |               | jump        |              |

    Plan du Scénario: Un tunnel en <authentification> incomplet ou non pris en charge rend la connexion <statut>
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" passe par un tunnel SSH en "<authentification>" avec la clé "<clé>", l'utilisateur SSH "<utilisateur>" et le mot de passe SSH "<mot de passe>"
      Quand l'agent liste les connexions
      Alors la connexion "tunnel-dev" est listée avec le statut "<statut>"
      Et la raison de la connexion "tunnel-dev" contient "<raison>"

      Exemples:
        | authentification | clé                 | utilisateur | mot de passe | statut      | raison                                              |
        | PUBLIC_KEY       | /absent/id_ed25519  | jump        |              | unavailable | SSH key file not found                              |
        | PUBLIC_KEY       |                     | jump        |              | unavailable | SSH key file not set in DBeaver                     |
        | PASSWORD         |                     | jump        |              | unavailable | no saved SSH password for this connection in DBeaver |
        | AGENT            |                     |             |              | unavailable | SSH user not saved in DBeaver                       |
        | KEYBOARD         |                     | jump        |              | unsupported | SSH authentication type KEYBOARD is not supported   |

    Plan du Scénario: Un gestionnaire réseau « <gestionnaire> » rend la connexion non prise en charge
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" a le gestionnaire réseau suivant :
        """
        <gestionnaire>
        """
      Quand l'agent liste les connexions
      Alors la connexion "tunnel-dev" est listée comme non prise en charge
      Et la raison de la connexion "tunnel-dev" contient "<raison>"

      Exemples:
        | gestionnaire                                                                                                    | raison                                  |
        | {"ssh_tunnel": {"type": "TUNNEL", "enabled": true, "properties": {"host": "bastion.example.com", "jumpServer.count": 1}}} | SSH jump servers are not supported yet  |
        | {"socks_proxy": {"type": "PROXY", "enabled": true, "properties": {"socks-host": "proxy.example.com"}}}          | proxy not supported in this version     |

    Scénario: Un tunnel désactivé est ignoré
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" a le gestionnaire réseau suivant :
        """
        {"ssh_tunnel": {"type": "TUNNEL", "enabled": false, "properties": {"host": "bastion.example.com", "authType": "KEYBOARD"}}}
        """
      Quand l'agent liste les connexions
      Alors la connexion "tunnel-dev" est listée comme disponible
      Et la connexion "tunnel-dev" n'a pas de tunnel SSH

  Règle: Une vérification de clé d'hôte contournée dans DBeaver est signalée dans la liste, et le tunnel n'y apparaît jamais

    Plan du Scénario: Un tunnel <vérification> est listé <drapeau>
      Étant donné un espace de travail DBeaver dont la connexion "tunnel-dev" passe par un tunnel SSH qui contourne la vérification de clé d'hôte : <contourne>
      Quand l'agent liste les connexions
      Alors la connexion "tunnel-dev" est listée comme disponible
      Et la vérification de clé d'hôte de la connexion "tunnel-dev" est listée comme "<drapeau>"
      Et la liste ne montre ni l'hôte "bastion.example.com", ni l'utilisateur "jump", ni le secret "s3cret"

      Exemples:
        | vérification | contourne | drapeau    |
        | contourné    | oui       | bypassed   |
        | vérifié      | non       | non signalé |

  Règle: Sans espace de travail DBeaver, la liste est vide et l'indice dit où il a été cherché

    Scénario: Un dossier DBeaver sans espace de travail donne une liste vide avec un indice
      Quand l'agent liste les connexions
      Alors la liste ne contient aucune connexion
      Et l'indice contient "no DBeaver workspace found"
      Et l'indice contient "SCARIF_DBEAVER_HOME"
