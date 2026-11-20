# language: fr
@connections
Fonctionnalité: Rebond SSH par alias
  Une connexion du fichier atteint sa base par un alias du fichier de configuration SSH, résolu comme le fait OpenSSH.

  @integration
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

  Scénario: Un alias qui déclare ProxyCommand est listé non pris en charge avec la directive nommée
    Étant donné un fichier de configuration SSH dont l'alias "bastion-cmd" déclare ProxyCommand
    Et le fichier de connexions suivant :
      """
      [[connection]]
      name = "via-command"
      host = "postgres"
      user = "reader"
      type = "dev"
      auth = "command"
      password_command = ["/bin/echo", "reader-secret"]
      ssh_alias = "bastion-cmd"
      """
    Quand l'agent liste les connexions
    Alors la connexion "via-command" est listée comme non prise en charge
    Et la raison de la connexion "via-command" contient "ProxyCommand"
