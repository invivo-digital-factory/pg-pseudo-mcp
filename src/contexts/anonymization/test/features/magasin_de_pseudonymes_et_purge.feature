# language: fr
@anonymization
Fonctionnalité: Magasin de pseudonymes et purge
  Le poste garde dans son répertoire de données un secret et un magasin qui relie chaque pseudonyme à sa valeur réelle,
  chiffrée avec ce secret. Ces fichiers ne sont lisibles que par l'utilisateur, ne contiennent aucune valeur réelle en
  clair, et la purge les supprime : les pseudonymes déjà donnés à un agent ne se traduisent alors plus.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Le magasin ne contient aucune valeur réelle en clair

    Scénario: Les valeurs réelles derrière cent pseudonymes n'apparaissent nulle part en clair dans le magasin
      Quand l'agent obtient le pseudonyme de 25 valeurs canari pour chacun des types "email, phone, last_name, person_name"
      Alors aucune de ces valeurs n'apparaît en clair dans les fichiers du magasin

  Règle: Le secret et le magasin ne sont lisibles que par leur propriétaire

    Scénario: Le secret et le magasin sont créés lisibles par leur seul propriétaire (droits 600)
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Alors les fichiers "secret, mapping.sqlite" du répertoire de données ont les droits 600
      Et le secret fait 32 octets

    Scénario: Un secret devenu lisible par d'autres est resserré à l'ouverture
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le secret du répertoire de données passe aux droits 644
      Et le serveur ouvre le répertoire de données
      Alors les fichiers "secret" du répertoire de données ont les droits 600

  Règle: Un magasin qui a perdu son secret, ou dont le secret est corrompu ou étranger, refuse de servir et oriente vers la purge

    Scénario: Sans son secret, le magasin refuse de s'ouvrir
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le secret du répertoire de données est supprimé
      Et le serveur ouvre le répertoire de données
      Alors l'ouverture échoue en conseillant "scarif-database-mcp mapping purge"

    Scénario: Un secret corrompu empêche l'ouverture
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le secret du répertoire de données est remplacé par 33 octets
      Et le serveur ouvre le répertoire de données
      Alors l'ouverture échoue en conseillant "scarif-database-mcp mapping purge"

    Scénario: Un magasin associé au secret d'un autre poste refuse de s'ouvrir
      Étant donné un second poste avec son propre répertoire de données
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le secret du second poste remplace celui du répertoire de données
      Et le serveur ouvre le répertoire de données
      Alors l'ouverture échoue en conseillant "scarif-database-mcp mapping purge"

  Règle: La purge supprime le magasin, ses fichiers de journalisation SQLite et le secret

    Scénario: La purge vide le répertoire de données
      Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
      Et le développeur purge la table de correspondance
      Alors le répertoire de données ne contient plus ni magasin, ni journal, ni secret

    Scénario: Une purge sur un répertoire vide réussit sans rien supprimer
      Quand le développeur purge la table de correspondance
      Alors le répertoire de données ne contient plus ni magasin, ni journal, ni secret

    Scénario: Après une purge, la même valeur reçoit un nouveau pseudonyme
      Quand l'agent obtient le pseudonyme de type "email" de "ylwenna.kervadec@canary-mail.fr"
      Et le développeur purge la table de correspondance
      Et l'agent obtient le pseudonyme de type "email" de "ylwenna.kervadec@canary-mail.fr"
      Alors le pseudonyme 2 diffère du pseudonyme 1
      Et le pseudonyme 2 se traduit en retour
