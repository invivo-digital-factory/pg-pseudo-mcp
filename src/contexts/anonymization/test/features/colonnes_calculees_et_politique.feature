# language: fr
@anonymization
Fonctionnalité: Colonnes calculées et politique de colonnes
  Une colonne calculée par la requête (concaténation, extraction, empreinte) n'a pas de nom fiable : quand elle lit
  une colonne personnelle, sa valeur devient un jeton opaque, sauf si un détecteur sait la pseudonymiser. L'équipe peut
  aussi déclarer une colonne en clair ou personnelle ; une colonne déclarée en clair garde toujours le filet des détecteurs
  structurels (e-mail, téléphone, IBAN), et une colonne déclarée personnelle prend le type donné par la politique.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Une colonne calculée à partir d'une colonne personnelle devient opaque si aucun détecteur ne la reconnaît

    Plan du Scénario: La colonne calculée "<colonne>" qui vaut "<valeur>" <résultat>
      Étant donné que la colonne calculée lit une colonne personnelle
      Quand l'agent lit la valeur "<valeur>" dans la colonne calculée "<colonne>"
      Alors <vérification>

      Exemples:
        | colonne    | valeur                              | résultat                              | vérification                                                   |
        | split_part | ylwenna.kervadec                    | devient un jeton opaque               | la valeur rendue est un jeton opaque "derived"                 |
        | line       | ylwenna.kervadec au 06 41 27 83 95  | devient un jeton opaque en entier     | la valeur rendue est un jeton opaque "derived"                 |
        | lower      | ylwenna.kervadec@canary-mail.fr     | reçoit un pseudonyme d'adresse e-mail | le pseudonyme est une adresse sur example.com, example.net ou example.org |
        | initials   | YK                                  | reste en clair, trop courte           | la valeur reste en clair                                       |

    Scénario: La colonne opaque est signalée avec la raison "derived"
      Étant donné que la colonne calculée lit une colonne personnelle
      Quand l'agent lit la valeur "ylwenna.kervadec" dans la colonne calculée "split_part"
      Alors la colonne est signalée "derived_personal" pour la raison "derived"

    Scénario: Une colonne calculée qui ne lit que des colonnes en clair garde ses valeurs
      Étant donné que la colonne calculée ne lit que des colonnes en clair
      Quand l'agent lit la valeur "PAID en_cours" dans la colonne calculée "label"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

  Règle: Une empreinte calculée devient opaque, une empreinte stockée reste en clair

    Scénario: Un hachage calculé par la requête devient un jeton opaque
      Quand l'agent lit la valeur "5d41402abc4b2a76b9719d911017c592" dans la colonne calculée "md5"
      Alors la valeur rendue est un jeton opaque "digest"
      Et la colonne est signalée pour la raison "derived"

    Scénario: Une somme de contrôle lue dans une colonne de la table reste en clair
      Quand l'agent lit la valeur "5d41402abc4b2a76b9719d911017c592" dans la colonne "checksum"
      Alors la valeur reste en clair

  Règle: Une colonne calculée prouvée en clair garde ses valeurs, sauf si son nom est personnel

    Scénario: Une concaténation prouvée en clair garde un nom de produit
      Étant donné que la colonne calculée est prouvée en clair
      Quand l'agent lit la valeur "Garance" dans la colonne calculée "concat"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

    Scénario: Une colonne calculée nommée "nom" reste pseudonymisée malgré la preuve
      Étant donné que la colonne calculée est prouvée en clair
      Quand l'agent lit la valeur "Kervadec" dans la colonne calculée "nom"
      Alors la valeur est pseudonymisée

  Règle: Une colonne déclarée en clair garde ses noms mais jamais une adresse e-mail, un téléphone ou un IBAN

    Scénario: Sans politique, un nom de produit est pris pour un nom de personne
      Quand l'agent lit la valeur "Garance Bleue" dans la colonne "libelle"
      Alors la valeur est pseudonymisée

    Scénario: Déclarée en clair, la colonne rend le nom de produit tel quel
      Étant donné que la politique déclare la colonne en clair
      Quand l'agent lit la valeur "Garance Bleue" dans la colonne "libelle"
      Alors la valeur reste en clair
      Et la colonne n'est pas signalée

    Plan du Scénario: Déclarée en clair, la colonne remplace quand même <type>
      Étant donné que la politique déclare la colonne en clair
      Quand l'agent lit la valeur "<valeur>" dans la colonne "libelle"
      Alors la valeur rendue ne contient plus "<fragment>"
      Et la valeur rendue contient "Garance "
      Et la colonne est signalée "<kind>" pour la raison "detected"

      Exemples:
        | type                | valeur                                          | fragment                        | kind  |
        | une adresse e-mail  | Garance ylwenna.kervadec@canary-mail.fr         | ylwenna.kervadec@canary-mail.fr | email |
        | un téléphone        | Garance 06 41 27 83 95                          | 06 41 27 83 95                  | phone |
        | un IBAN             | Garance FR76 3000 4000 0312 3456 7890 143       | 3000 4000 0312                  | iban  |

  Règle: Une colonne déclarée personnelle est pseudonymisée avec le type donné par la politique

    Plan du Scénario: La colonne "<colonne>" déclarée <type> remplace "<valeur>"
      Étant donné que la politique déclare la colonne personnelle comme "<type>"
      Quand l'agent lit "<valeur>" dans la colonne "<colonne>" de type "<type sql>"
      Alors la valeur est pseudonymisée
      Et la colonne est signalée "<type>" pour la raison "policy"

      Exemples:
        | colonne  | type sql | valeur    | type      |
        | code_lot | text     | lot-42    | last_name |
        | numero   | int8     | 418273652 | siren     |
