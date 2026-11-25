# language: fr
@connections
Fonctionnalité: Rebonds SSH (ProxyJump)
  Un alias SSH peut atteindre son bastion à travers une chaîne de rebonds ProxyJump, résolue par OpenSSH et parcourue par le serveur lui-même.
  Chaque saut vérifie sa propre clé d'hôte ; la chaîne est bornée à huit rebonds et ne peut pas boucler.

  Règle: La chaîne suit le ProxyJump de l'alias, puis celui du premier saut, jusqu'à huit rebonds

    Plan du Scénario: Un alias à <rebonds> rebonds successifs traverse <sauts> sauts
      Étant donné un fichier de configuration SSH où l'alias "bastion" atteint son bastion par <rebonds> rebonds successifs
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible
      Et la connexion "via-bastion" traverse <sauts> sauts SSH

      Exemples:
        | rebonds | sauts |
        | 1       | 2     |
        | 8       | 9     |

    Scénario: Neuf rebonds successifs sont refusés
      Étant donné un fichier de configuration SSH où l'alias "bastion" atteint son bastion par 9 rebonds successifs
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme non prise en charge
      Et la raison de la connexion "via-bastion" contient "the ProxyJump chain is longer than 8 hops"

    Scénario: Une chaîne qui boucle est refusée sans nommer d'hôte
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          ProxyJump relais
        Host relais
          HostName relais.example.com
          ProxyJump bastion
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme non prise en charge
      Et la raison de la connexion "via-bastion" contient "the ProxyJump chain loops back on itself"
      Et la raison de la connexion "via-bastion" ne contient pas "example.com"

    Scénario: Une liste ProxyJump est parcourue dans l'ordre, sans suivre le ProxyJump des sauts suivants
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          ProxyJump alice@premier.example.com:2222,second
        Host second
          HostName second.example.com
          ProxyJump ignore.example.com
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible
      Et la connexion "via-bastion" traverse les sauts SSH "premier.example.com:2222, second.example.com:22, bastion.example.com:22"

  Règle: Un saut ProxyJump littéral n'a pas besoin de bloc Host, mais l'alias de l'entrée oui

    Scénario: Un rebond écrit en clair dans ProxyJump suffit
      Étant donné un fichier de configuration SSH qui contient :
        """
        Host bastion
          HostName bastion.example.com
          ProxyJump relais.example.com
        """
      Et une connexion du fichier "via-bastion" derrière l'alias SSH "bastion"
      Quand l'agent liste les connexions
      Alors la connexion "via-bastion" est listée comme disponible
      Et la connexion "via-bastion" traverse 2 sauts SSH

  @integration
  Règle: Chaque saut vérifie sa propre clé d'hôte, et un refus nomme le saut par sa position et l'alias

    Scénario: Une chaîne à deux sauts dont les clés sont connues ouvre la session
      Étant donné une connexion du fichier "via-rebond" vers la base de test derrière l'alias "app-via-jump" qui rebondit par le bastion de test
      Et un fichier known_hosts qui connaît la clé du bastion à ses deux sauts
      Quand l'agent exécute une requête de lecture sur la connexion "via-rebond"
      Alors la session s'ouvre et répond

    Scénario: Une clé inconnue au premier saut est refusée avec la position du saut, l'alias et l'empreinte
      Étant donné une connexion du fichier "via-rebond" vers la base de test derrière l'alias "app-via-jump" qui rebondit par le bastion de test
      Et un fichier known_hosts qui ne connaît la clé du bastion qu'à son second saut
      Quand l'agent exécute une requête de lecture sur la connexion "via-rebond"
      Alors la connexion est refusée
      Et le message de refus contient "SSH hop 1 of 2 (ProxyJump hop 1 of SSH alias app-via-jump)"
      Et le message de refus contient "not in known_hosts"
      Et le message de refus contient l'empreinte "SHA256:" de la clé du bastion

    Scénario: Une clé inconnue au second saut est refusée même avec StrictHostKeyChecking no
      Étant donné une connexion du fichier "via-rebond" vers la base de test derrière l'alias "app-via-jump" qui rebondit par le bastion de test en StrictHostKeyChecking no
      Et un fichier known_hosts qui ne connaît la clé du bastion qu'à son premier saut
      Quand l'agent exécute une requête de lecture sur la connexion "via-rebond"
      Alors la connexion est refusée
      Et le message de refus contient "SSH hop 2 of 2"
      Et le message de refus contient l'empreinte "SHA256:" de la clé du bastion
