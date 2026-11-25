# language: fr
@connections
Fonctionnalité: Vérification des clés d'hôte SSH
  Un bastion n'est joint que si sa clé d'hôte est déjà connue de l'utilisateur dans known_hosts, à chaque saut et quelle que soit la configuration SSH.
  Une clé inconnue, changée ou révoquée refuse la connexion avec l'empreinte de la clé, pour que l'utilisateur la vérifie lui-même.

  @integration
  Règle: Seule une clé d'hôte connue ouvre la session

    Plan du Scénario: Un known_hosts <entrée> ouvre la session
      Étant donné une connexion DBeaver qui traverse le bastion SSH de test
      Et un fichier known_hosts <entrée>
      Quand l'agent exécute une requête de lecture sur la connexion tunnelée
      Alors la session s'ouvre et répond

      Exemples:
        | entrée                                                             |
        | qui connaît la clé du bastion                                      |
        | qui connaît la clé du bastion sous une forme hachée                |
        | qui connaît la clé du bastion après la clé d'un autre hôte         |

    Scénario: Un bastion inconnu est refusé avec son empreinte et la commande pour l'ajouter
      Étant donné une connexion DBeaver qui traverse le bastion SSH de test
      Et un fichier known_hosts vide
      Quand l'agent exécute une requête de lecture sur la connexion tunnelée
      Alors la connexion est refusée
      Et le message de refus contient "not in known_hosts"
      Et le message de refus contient "ssh-keyscan -p"
      Et le message de refus contient l'empreinte "SHA256:" de la clé du bastion

    Plan du Scénario: Un known_hosts <entrée> refuse le bastion
      Étant donné une connexion DBeaver qui traverse le bastion SSH de test
      Et un fichier known_hosts <entrée>
      Quand l'agent exécute une requête de lecture sur la connexion tunnelée
      Alors la connexion est refusée
      Et le message de refus contient "<motif>"
      Et le message de refus contient l'empreinte "SHA256:" de la clé du bastion

      Exemples:
        | entrée                                                    | motif                |
        | qui porte une autre clé pour le bastion                   | changed              |
        | qui connaît la clé du bastion mais la révoque aussi       | is revoked by line 2 |
        | qui ne déclare la clé du bastion que comme autorité de certification | not in known_hosts |

  @integration
  Règle: StrictHostKeyChecking no dans la configuration SSH ne désactive pas la vérification

    Scénario: Un alias en StrictHostKeyChecking no vers un bastion inconnu est refusé
      Étant donné un fichier de configuration SSH de test avec l'alias "bastion-test" en StrictHostKeyChecking no
      Et un fichier known_hosts vide
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "via-bastion"
        host = "postgres"
        database = "{base}"
        user = "{utilisateur}"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "{mot_de_passe}"]
        ssh_alias = "bastion-test"
        """
      Quand l'agent exécute une requête de lecture sur la connexion "via-bastion"
      Alors la connexion est refusée
      Et le message de refus contient "not in known_hosts"

  Règle: Seul DBeaver peut contourner la vérification, et la liste le signale

    @integration
    Scénario: Un tunnel DBeaver qui contourne la vérification ouvre la session sans known_hosts
      Étant donné une connexion DBeaver qui traverse le bastion SSH de test en contournant la vérification de clé d'hôte
      Et un fichier known_hosts vide
      Quand l'agent exécute une requête de lecture sur la connexion tunnelée
      Alors la session s'ouvre et répond

    Scénario: Un tunnel DBeaver qui contourne la vérification est listé avec le drapeau bypassed
      Étant donné une connexion DBeaver qui traverse le bastion SSH de test en contournant la vérification de clé d'hôte
      Quand l'agent liste les connexions
      Alors la vérification de clé d'hôte de la connexion "canary-local" est listée comme "bypassed"
