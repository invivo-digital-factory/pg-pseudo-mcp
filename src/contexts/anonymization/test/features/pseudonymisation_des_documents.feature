# language: fr
@anonymization
Fonctionnalité: Pseudonymisation des documents
  Un document revient avec chaque valeur personnelle pseudonymisée selon le chemin de son champ, et la liste des chemins pseudonymisés dit pour chacun le type et la raison.

  Scénario: AE5, un client fait de valeurs canari revient pseudonymisé à ses quatre chemins personnels
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {
        "_id": {"$oid": "6650f1c2a4b3e2d1c0b9a8f7"},
        "firstName": "Ylwenna",
        "lastName": "Kervadec",
        "contact": {
          "email": "ylwenna.kervadec@canary-mail.fr",
          "phones": ["06 41 27 83 95", "07 58 14 92 36"]
        },
        "createdAt": {"$date": "2026-03-02T09:15:00.000Z"},
        "status": "PAID"
      }
      """
    Quand l'agent lit ce document
    Alors les chemins pseudonymisés sont exactement "contact.email, contact.phones, firstName, lastName"
    Et le chemin "firstName" est signalé "first_name" pour la raison "dictionary"
    Et le chemin "lastName" est signalé "last_name" pour la raison "dictionary"
    Et le chemin "contact.email" est signalé "email" pour la raison "dictionary"
    Et le chemin "contact.phones" est signalé "phone" pour la raison "detected"
    Et le champ "_id" reste en clair
    Et le champ "createdAt" reste en clair
    Et le champ "status" reste en clair
    Et la réponse ne contient aucune des valeurs "Ylwenna, Kervadec, ylwenna.kervadec@canary-mail.fr, 06 41 27 83 95, 07 58 14 92 36"

  Scénario: Un mot court capitalisé dans un champ inconnu est pseudonymisé par défaut
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"titulaire": "Kervadec", "etat": "PAID"}
      """
    Quand l'agent lit ce document
    Alors le champ "titulaire" est pseudonymisé
    Et le chemin "titulaire" est signalé "last_name" pour la raison "default"
    Et le champ "etat" reste en clair

  Scénario: Une table indexée par des adresses e-mail voit ses clés remplacées par leurs pseudonymes
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"abonnes": {"ylwenna.kervadec@canary-mail.fr": true, "o.hazvekmor@canary-mail.fr": false}}
      """
    Quand l'agent lit ce document
    Alors les clés du champ "abonnes" sont les pseudonymes de "ylwenna.kervadec@canary-mail.fr, o.hazvekmor@canary-mail.fr"
    Et la réponse ne contient aucune des valeurs "ylwenna.kervadec@canary-mail.fr, o.hazvekmor@canary-mail.fr"

  Scénario: Une date de naissance est décalée, une date de création ne l'est pas
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"birthDate": {"$date": "1987-03-14T00:00:00.000Z"}, "createdAt": {"$date": "2026-03-02T09:15:00.000Z"}}
      """
    Quand l'agent lit ce document
    Alors le champ "birthDate" est une autre date
    Et le chemin "birthDate" est signalé "birth_date" pour la raison "dictionary"
    Et le champ "createdAt" reste en clair

  Scénario: Un téléphone stocké en Int64 sous "telephone" est pseudonymisé, une quantité ne l'est pas
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"telephone": {"$numberLong": "33641278395"}, "quantity": {"$numberLong": "33641278395"}}
      """
    Quand l'agent lit ce document
    Alors le champ "telephone" est pseudonymisé
    Et le chemin "telephone" est signalé "phone" pour la raison "dictionary"
    Et le champ "quantity" reste en clair

  Scénario: La même adresse e-mail reçoit le même pseudonyme dans une ligne PostgreSQL et dans un document
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"customer": {"mail": "ylwenna.kervadec@canary-mail.fr"}}
      """
    Quand l'agent lit une ligne PostgreSQL dont la colonne "email" vaut "ylwenna.kervadec@canary-mail.fr"
    Et l'agent lit ce document
    Alors le champ "customer.mail" porte le pseudonyme de la ligne

  Scénario: Un champ déclaré personnel est pseudonymisé avec le type donné par la politique
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"code_lot": "lot-42", "numero": 418273652}
      """
    Et la politique déclare le champ "code_lot" personnel comme "last_name"
    Et la politique déclare le champ "numero" personnel comme "siren"
    Quand l'agent lit ce document
    Alors le champ "code_lot" est pseudonymisé
    Et le chemin "code_lot" est signalé "last_name" pour la raison "policy"
    Et le champ "numero" est pseudonymisé
    Et le chemin "numero" est signalé "siren" pour la raison "policy"

  Scénario: Un champ déclaré en clair garde les noms mais remplace une adresse e-mail
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"nom": "Garance Bleue", "libelle": "Garance ylwenna.kervadec@canary-mail.fr"}
      """
    Et la politique déclare le champ "nom" en clair
    Et la politique déclare le champ "libelle" en clair
    Quand l'agent lit ce document
    Alors le champ "nom" reste en clair
    Et le chemin "libelle" est signalé "email" pour la raison "detected"
    Et la réponse ne contient aucune des valeurs "ylwenna.kervadec@canary-mail.fr"

  Scénario: Les identifiants, booléens et nuls restent en clair même sous un nom personnel
    Étant donné un répertoire de données neuf
    Et le document suivant :
      """
      {"nom": [{"$oid": "6650f1c2a4b3e2d1c0b9a8f7"}, true, null]}
      """
    Quand l'agent lit ce document
    Alors le champ "nom" reste en clair
    Et les chemins pseudonymisés sont exactement ""

  Scénario: Au-delà de deux cents chemins, la liste des chemins pseudonymisés est tronquée
    Étant donné un répertoire de données neuf
    Et un document de 250 champs inconnus valant "Kervadec"
    Quand l'agent lit ce document
    Alors la liste compte 200 chemins pseudonymisés et 50 chemins omis
    Et la réponse ne contient aucune des valeurs "Kervadec"
