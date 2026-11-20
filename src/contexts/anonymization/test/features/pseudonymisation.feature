# language: fr
@anonymization
Fonctionnalité: Pseudonymisation des valeurs personnelles
  Une valeur personnelle reçoit toujours le même pseudonyme, que seul le poste local sait traduire en retour.

  Scénario: La même adresse e-mail donne le même pseudonyme, sur un domaine réservé
    Étant donné un répertoire de données neuf
    Quand l'agent obtient deux fois le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
    Alors les deux pseudonymes sont identiques
    Et le pseudonyme n'est pas l'adresse réelle
    Et le pseudonyme est sur un domaine réservé

  Scénario: Un pseudonyme se traduit en retour vers la valeur réelle, en local
    Étant donné un répertoire de données neuf
    Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
    Et l'agent traduit ce pseudonyme
    Alors la traduction rend l'adresse "ylwenna.kervadec@canary-mail.fr"

  Scénario: Après une purge, un ancien pseudonyme ne se traduit plus
    Étant donné un répertoire de données neuf
    Quand l'agent obtient le pseudonyme de l'adresse "ylwenna.kervadec@canary-mail.fr"
    Et le développeur purge la table de correspondance
    Et l'agent traduit ce pseudonyme
    Alors la traduction ne rend aucune valeur réelle
