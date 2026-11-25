# language: fr
@connections
Fonctionnalité: Atteindre une base par un alias SSH
  Une connexion du fichier traverse un bastion en nommant, avec ssh_alias, un Host de la configuration OpenSSH ; ssh -G en résout l'hôte, le port, l'utilisateur, les clés et les rebonds.
  L'alias doit être défini par un bloc Host, et une directive que le serveur ne sait pas honorer rend la connexion non prise en charge au lieu d'être ignorée.

  Règle: L'alias doit correspondre à un bloc Host autre qu'un simple *, avec les jokers et négations d'OpenSSH

    Plan du Scénario: L'alias "bastion" est défini par « <bloc> »
      Étant donné un fichier de configuration SSH qui contient :
        """
        <bloc>
          HostName bastion.example.com
          User jump
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible

      Exemples:
        | bloc                  |
        | Host bastion          |
        | Host bast*            |
        | Host b?stion          |
        | Host other bastion    |
        | Host="bastion"        |
        | Host bastion !other   |

    Plan du Scénario: L'alias "bastion" n'est pas défini par « <bloc> »
      Étant donné un fichier de configuration SSH qui contient :
        """
        <bloc>
          HostName bastion.example.com
          User jump
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme indisponible
      Et la raison de la connexion "via-bastion" contient "SSH alias bastion is not defined in the OpenSSH config; add a Host block for it"

      Exemples:
        | bloc                  |
        | Host *                |
        | Host bast* !bastion   |
        | Host other            |
        | Match host bastion    |

    Scénario: Un alias défini dans un fichier inclus est défini
      Étant donné un fichier de configuration SSH qui inclut un fichier où est défini l'alias "bastion"
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible

    Scénario: Un alias SSH absent de la configuration SSH rend la connexion indisponible avec une raison qui le nomme
      Étant donné un fichier de configuration SSH de test qui ne définit pas l'alias "bastion-absent"
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "via-absent"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        ssh_alias = "bastion-absent"
        """
      Quand l'agent liste les connexions
      Alors la connexion "via-absent" est listée comme indisponible
      Et la raison de la connexion "via-absent" contient "SSH alias bastion-absent is not defined in the OpenSSH config"
      Et la raison de la connexion "via-absent" contient "add a Host block"
      Et la liste ne montre ni l'hôte "db.example.com", ni l'utilisateur "alice", ni le secret "s3cret"

  Règle: Une directive que le serveur ne sait pas honorer rend la connexion non prise en charge et la nomme

    Plan du Scénario: La directive « <directive> » rend la connexion non prise en charge
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          User jump
          <directive>
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme non prise en charge
      Et la raison de la connexion "via-bastion" contient "<raison>"

      Exemples:
        | directive                             | raison                                                       |
        | ProxyCommand nc %h %p                 | the SSH directive ProxyCommand is not supported              |
        | ProxyUseFdpass yes                    | the SSH directive ProxyUseFdpass is not supported            |
        | KnownHostsCommand /bin/true           | the SSH directive KnownHostsCommand is not supported         |
        | CertificateFile ~/.ssh/absent-cert.pub | the SSH directive CertificateFile is not supported          |
        | PKCS11Provider /absent/pkcs11.so      | the SSH directive PKCS11Provider is not supported            |
        | CanonicalizeHostname yes              | the SSH directive CanonicalizeHostname is not supported      |
        | BindAddress 127.0.0.1                 | the SSH directive BindAddress is not supported               |
        | BindInterface lo0                     | the SSH directive BindInterface is not supported             |
        | IdentityFile ~/.ssh/%C                | the SSH directive IdentityFile uses the unsupported token %C |

    Plan du Scénario: La directive « <directive> » à sa valeur neutre ne bloque pas la connexion
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          User jump
          <directive>
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible

      Exemples:
        | directive                    |
        | ProxyCommand none            |
        | CanonicalizeHostname no      |
        | IdentityFile ~/.ssh/id_%r@%h |

  Règle: StrictHostKeyChecking no ne désactive jamais la vérification des clés d'hôte

    Scénario: Un alias en StrictHostKeyChecking no reste listé sans contournement de la vérification
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          User jump
          StrictHostKeyChecking no
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible
      Et la vérification de clé d'hôte de la connexion "via-bastion" est listée comme "non signalé"

  Règle: Sans client OpenSSH, seules les connexions qui nomment un alias sont indisponibles

    Scénario: Sans ssh sur le PATH une connexion à alias est indisponible et les autres restent disponibles
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
        """
      Et le client OpenSSH n'est pas installé
      Et le fichier de connexions suivant :
        """
        [[connection]]
        name = "direct"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]

        [[connection]]
        name = "via-bastion"
        host = "db.example.com"
        user = "alice"
        type = "dev"
        auth = "command"
        password_command = ["/bin/echo", "s3cret"]
        ssh_alias = "bastion"
        """
      Quand l'agent liste les connexions
      Alors la connexion "direct" est listée comme disponible
      Et la connexion "via-bastion" est listée comme indisponible
      Et la raison de la connexion "via-bastion" contient "the OpenSSH client (ssh) was not found on PATH"

  @integration
  Règle: À travers l'alias, la session s'ouvre jusqu'à la base

    Scénario: La session s'ouvre à travers l'alias SSH du fichier de configuration de test
      Étant donné un fichier de configuration SSH de test avec l'alias "bastion-test"
      Et un fichier known_hosts qui connaît la clé du bastion
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
      Alors la session s'ouvre et répond
