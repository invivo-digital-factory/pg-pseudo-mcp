# language: fr
@anonymization
Fonctionnalité: Pseudonymisation des documents
  Un document MongoDB revient avec chaque valeur personnelle pseudonymisée selon le chemin de son champ : le segment
  connu du dictionnaire le plus proche donne le type, les tableaux héritent du chemin de leur champ, les détecteurs veillent sous les clés
  inconnues. La liste des chemins pseudonymisés dit pour chacun le type et la raison, sans indice de tableau.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Le nom du champ donne le type de sa valeur

    Scénario: Un client revient pseudonymisé à ses quatre chemins personnels, le reste en clair
      Étant donné le document suivant :
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
      Et le chemin "contact.phones" est signalé "phone" pour la raison "dictionary"
      Et le champ "_id" reste en clair
      Et le champ "createdAt" reste en clair
      Et le champ "status" reste en clair
      Et la réponse ne contient aucune des valeurs "Ylwenna, Kervadec, ylwenna.kervadec@canary-mail.fr, 06 41 27 83 95, 07 58 14 92 36"

    Plan du Scénario: Le champ au pluriel "<champ>" prend le type <type> de son singulier
      Étant donné le document suivant :
        """
        {"<champ>": [<valeur>]}
        """
      Quand l'agent lit ce document
      Alors le champ "<champ>" est pseudonymisé
      Et le chemin "<champ>" est signalé "<type>" pour la raison "dictionary"

      Exemples:
        | champ        | valeur                | type        |
        | phones       | {"$numberLong": "33641278395"} | phone |
        | prenoms      | "ylwenna"             | first_name  |
        | codesPostaux | 29280                 | postal_code |

    Scénario: Un numéro sous une adresse prend le type adresse, un identifiant ne le prend pas
      Étant donné le document suivant :
        """
        {"adresse": {"numero": 12, "id": "ABC-12"}}
        """
      Quand l'agent lit ce document
      Alors le chemin "adresse.numero" est signalé "address" pour la raison "dictionary"
      Et le champ "adresse.id" reste en clair

    Scénario: Un téléphone stocké en Int64 sous "telephone" est pseudonymisé, une quantité ne l'est pas
      Étant donné le document suivant :
        """
        {"telephone": {"$numberLong": "33641278395"}, "quantity": {"$numberLong": "33641278395"}}
        """
      Quand l'agent lit ce document
      Alors le champ "telephone" est pseudonymisé
      Et le chemin "telephone" est signalé "phone" pour la raison "dictionary"
      Et le champ "quantity" reste en clair

    Scénario: Un prix à virgule reste en clair, même sous un nom de téléphone
      Étant donné le document suivant :
        """
        {"telephoneSecours": 12.5, "prix": 12.5}
        """
      Quand l'agent lit ce document
      Alors le champ "telephoneSecours" reste en clair
      Et le champ "prix" reste en clair

  Règle: Un tableau hérite du chemin de son champ, sans indice

    Scénario: Les éléments d'un tableau d'objets sont signalés sous un seul chemin
      Étant donné le document suivant :
        """
        {"items": [{"nom": "Kervadec", "sku": "A-12"}, {"nom": "Hazvekmor", "sku": "B-7"}]}
        """
      Quand l'agent lit ce document
      Alors les chemins pseudonymisés sont exactement "items.nom"
      Et le chemin "items.nom" est signalé "last_name" pour la raison "dictionary"
      Et la réponse ne contient aucune des valeurs "Kervadec, Hazvekmor"
      Et la réponse contient encore les valeurs "A-12, B-7"

  Règle: Sous une clé inconnue, les détecteurs et la forme d'un nom veillent

    Scénario: Un mot court capitalisé dans un champ inconnu est pseudonymisé par défaut
      Étant donné le document suivant :
        """
        {"titulaire": "Kervadec", "etat": "PAID"}
        """
      Quand l'agent lit ce document
      Alors le champ "titulaire" est pseudonymisé
      Et le chemin "titulaire" est signalé "last_name" pour la raison "default"
      Et le champ "etat" reste en clair

    Scénario: Une adresse e-mail sous des clés inconnues est détectée
      Étant donné le document suivant :
        """
        {"contact": {"perso": {"courrier": "ylwenna.kervadec@canary-mail.fr"}}}
        """
      Quand l'agent lit ce document
      Alors le chemin "contact.perso.courrier" est signalé "email" pour la raison "detected"
      Et la réponse ne contient aucune des valeurs "ylwenna.kervadec@canary-mail.fr"

    Scénario: Une table indexée par des adresses e-mail voit ses clés remplacées par leurs pseudonymes
      Étant donné le document suivant :
        """
        {"abonnes": {"ylwenna.kervadec@canary-mail.fr": true, "o.hazvekmor@canary-mail.fr": false}}
        """
      Quand l'agent lit ce document
      Alors les clés du champ "abonnes" sont les pseudonymes de "ylwenna.kervadec@canary-mail.fr, o.hazvekmor@canary-mail.fr"
      Et la réponse ne contient aucune des valeurs "ylwenna.kervadec@canary-mail.fr, o.hazvekmor@canary-mail.fr"

  Règle: Les types BSON neutres restent en clair, les binaires deviennent opaques

    Scénario: Les identifiants, booléens et nuls restent en clair même sous un nom personnel
      Étant donné le document suivant :
        """
        {"nom": [{"$oid": "6650f1c2a4b3e2d1c0b9a8f7"}, true, null]}
        """
      Quand l'agent lit ce document
      Alors le champ "nom" reste en clair
      Et les chemins pseudonymisés sont exactement ""

    Scénario: Un binaire n'est jamais montré, seule sa longueur l'est
      Étant donné le document suivant :
        """
        {"piece": {"$binary": {"base64": "WWx3ZW5uYSBLZXJ2", "subType": "00"}}}
        """
      Quand l'agent lit ce document
      Alors le champ "piece" vaut "<binary len=12>"
      Et le chemin "piece" est signalé "person_name" pour la raison "default"

  Règle: Une date n'est décalée que si c'est une date de naissance

    Scénario: Une date de naissance est décalée, une date de création ne l'est pas
      Étant donné le document suivant :
        """
        {"birthDate": {"$date": "1987-03-14T00:00:00.000Z"}, "createdAt": {"$date": "2026-03-02T09:15:00.000Z"}}
        """
      Quand l'agent lit ce document
      Alors le champ "birthDate" est une autre date
      Et le chemin "birthDate" est signalé "birth_date" pour la raison "dictionary"
      Et le champ "createdAt" reste en clair

  Règle: Une valeur reçoit le même pseudonyme dans une ligne PostgreSQL et dans un document

    Scénario: La même adresse e-mail reçoit le même pseudonyme dans une ligne PostgreSQL et dans un document
      Étant donné le document suivant :
        """
        {"customer": {"mail": "ylwenna.kervadec@canary-mail.fr"}}
        """
      Quand l'agent lit une ligne PostgreSQL dont la colonne "email" vaut "ylwenna.kervadec@canary-mail.fr"
      Et l'agent lit ce document
      Alors le champ "customer.mail" porte le pseudonyme de la ligne

  Règle: La politique de champs déclare un champ personnel, en clair ou renommé

    Scénario: Un champ déclaré personnel est pseudonymisé avec le type donné par la politique
      Étant donné le document suivant :
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

    Scénario: Un champ déclaré personnel couvre tout son sous-arbre, sauf un enfant déclaré en clair
      Étant donné le document suivant :
        """
        {"dossier": {"piece": {"reference": "lot-42"}, "libelle": "Garance Bleue"}}
        """
      Et la politique déclare le champ "dossier" personnel comme "last_name"
      Et la politique déclare le champ "dossier.libelle" en clair
      Quand l'agent lit ce document
      Alors le chemin "dossier.piece.reference" est signalé "last_name" pour la raison "policy"
      Et le champ "dossier.libelle" reste en clair

    Scénario: Un champ déclaré en clair garde les noms mais remplace une adresse e-mail
      Étant donné le document suivant :
        """
        {"nom": "Garance Bleue", "libelle": "Garance ylwenna.kervadec@canary-mail.fr"}
        """
      Et la politique déclare le champ "nom" en clair
      Et la politique déclare le champ "libelle" en clair
      Quand l'agent lit ce document
      Alors le champ "nom" reste en clair
      Et le chemin "libelle" est signalé "email" pour la raison "detected"
      Et la réponse ne contient aucune des valeurs "ylwenna.kervadec@canary-mail.fr"

    Scénario: Un objet déclaré en clair ne couvre pas ses enfants
      Étant donné le document suivant :
        """
        {"contact": {"nom": "Kervadec"}}
        """
      Et la politique déclare le champ "contact" en clair
      Quand l'agent lit ce document
      Alors le chemin "contact.nom" est signalé "last_name" pour la raison "dictionary"

    Scénario: Un champ renommé par un pipeline garde le type du nom de sa source
      Étant donné le document suivant :
        """
        {"n": {"$numberLong": "33641278395"}, "quantite": 2}
        """
      Et la politique déclare le champ "n" renommé depuis "telephone"
      Et la politique déclare le champ "quantite" renommé depuis "quantity"
      Quand l'agent lit ce document
      Alors le chemin "n" est signalé "phone" pour la raison "dictionary"
      Et le champ "quantite" reste en clair

    Scénario: Un champ calculé à partir d'une donnée personnelle devient opaque
      Étant donné le document suivant :
        """
        {"who": "ylwenna.kervadec", "n": 31}
        """
      Et la politique déclare le champ "who" calculé à partir d'une donnée personnelle
      Et la politique déclare le champ "n" calculé à partir d'une donnée personnelle
      Quand l'agent lit ce document
      Alors le champ "who" est un jeton opaque "derived"
      Et le chemin "who" est signalé "derived_personal" pour la raison "derived"
      Et le champ "n" reste en clair

  Règle: La liste des chemins pseudonymisés est bornée

    Scénario: Au-delà de deux cents chemins, la liste des chemins pseudonymisés est tronquée
      Étant donné un document de 250 champs inconnus valant "Kervadec"
      Quand l'agent lit ce document
      Alors la liste compte 200 chemins pseudonymisés et 50 chemins omis
      Et la réponse ne contient aucune des valeurs "Kervadec"
