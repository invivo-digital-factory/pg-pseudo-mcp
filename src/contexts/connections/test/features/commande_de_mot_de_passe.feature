# language: fr
@connections
Fonctionnalité: Mot de passe donné par une commande
  En mode command, le mot de passe est la sortie standard de password_command, un programme suivi de ses arguments, lancé sans shell : un trousseau ou un gestionnaire de secrets.
  La commande ne tourne qu'à l'ouverture d'une session, jamais pour lister ; sa sortie reste en mémoire et n'apparaît dans aucune réponse.

  Règle: Lister les connexions n'exécute jamais la commande

    Scénario: Lister les connexions n'exécute pas la commande de mot de passe
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app-prod"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/sh", "-c", "touch {dossier}/marqueur"]
        """
      Quand l'agent liste les connexions
      Alors la connexion "app-prod" est listée comme disponible
      Et la commande de mot de passe n'a pas été exécutée

  Règle: Le mot de passe est la sortie de la commande privée d'un seul saut de ligne final

    Plan du Scénario: La sortie « <sortie> » donne le mot de passe « <mot de passe> »
      Quand le serveur lance la commande de mot de passe ["/usr/bin/printf", "<sortie>"]
      Alors la commande donne le mot de passe « <mot de passe> »

      Exemples:
        | sortie       | mot de passe |
        | s3cret       | s3cret       |
        | s3cret\\n    | s3cret       |
        | s3cret\\r\\n | s3cret       |
        | s3cret\\n\\n | s3cret⏎      |
        | s3 cret\\n   | s3 cret      |

    Scénario: Les arguments sont passés tels quels, sans shell
      Quand le serveur lance la commande de mot de passe ["/bin/echo", "$HOME", "*", "a;b"]
      Alors la commande donne le mot de passe « $HOME * a;b »

  Règle: Une commande en échec donne un motif fixe, jamais sa sortie

    Plan du Scénario: Une commande qui <échec> est refusée avec le motif « <motif> »
      Quand le serveur lance la commande de mot de passe <commande>
      Alors la commande échoue avec le motif « <motif> »

      Exemples:
        | échec                         | commande                                                                  | motif                                               |
        | sort avec le statut 44        | ["/bin/sh", "-c", "echo out-leak-4411; echo err-leak-4411 >&2; exit 44"] | the password command exited with status 44          |
        | est introuvable               | ["/absent/scarif-password-command"]                                       | the password command was not found                  |
        | n'est pas exécutable          | ["/etc/hosts"]                                                            | the password command could not be started           |
        | n'écrit rien                  | ["/bin/sh", "-c", "exit 0"]                                               | the password command printed nothing                |
        | n'écrit qu'un saut de ligne   | ["/bin/echo"]                                                             | the password command printed nothing                |
        | n'écrit pas de l'UTF-8        | ["/usr/bin/printf", "\\377"]                                              | the password command output is not valid UTF-8      |
        | est tuée par un signal        | ["/bin/sh", "-c", "kill -9 $$"]                                           | the password command was stopped by a signal        |

    Scénario: Une commande qui dépasse son délai, 30 secondes en service, est interrompue
      Quand le serveur lance la commande de mot de passe ["/bin/sleep", "5"] avec un délai réduit à 1 seconde
      Alors la commande échoue avec le motif « the password command did not finish in time »

    @ae3
    Scénario: Une commande de mot de passe qui échoue rend un motif fixe avec son statut et sans sa sortie
      Étant donné le fichier de connexions suivant :
        """
        [[connection]]
        name = "app-prod"
        host = "db.example.com"
        user = "alice"
        type = "prod"
        auth = "command"
        password_command = ["/bin/sh", "-c", "echo out-leak-4411; echo err-leak-4411 >&2; exit 44"]
        """
      Quand l'agent exécute une requête de lecture sur la connexion "app-prod"
      Alors la requête échoue avec une erreur qui contient "status 44"
      Et l'erreur de la requête ne contient pas "leak"

  @integration
  Règle: La commande tourne une fois par session, et une seule fois de plus après un refus d'authentification

    Scénario: Une session ouverte sert les requêtes suivantes sans relancer la commande
      Étant donné une connexion du fichier "app" vers la base de test dont la commande de mot de passe donne d'abord « le bon mot de passe » puis « le bon mot de passe »
      Quand l'agent exécute deux requêtes de lecture successives sur la connexion "app"
      Alors la session s'ouvre et répond
      Et la commande de mot de passe a été exécutée 1 fois

    Scénario: Un mot de passe refusé relance la commande exactement une fois
      Étant donné une connexion du fichier "app" vers la base de test dont la commande de mot de passe donne d'abord « perime-5521 » puis « le bon mot de passe »
      Quand l'agent exécute une requête de lecture sur la connexion "app"
      Alors la session s'ouvre et répond
      Et la commande de mot de passe a été exécutée 2 fois

    Scénario: Deux mots de passe refusés échouent sans troisième exécution ni fuite
      Étant donné une connexion du fichier "app" vers la base de test dont la commande de mot de passe donne d'abord « perime-5521 » puis « perime-5521 »
      Quand l'agent exécute une requête de lecture sur la connexion "app"
      Alors la requête échoue avec une erreur qui contient "28P01"
      Et l'erreur de la requête ne contient pas "perime-5521"
      Et l'erreur de la requête ne contient ni l'utilisateur ni l'hôte de la base de test
      Et la commande de mot de passe a été exécutée 2 fois
