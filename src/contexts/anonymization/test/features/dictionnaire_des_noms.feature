# language: fr
@anonymization
Fonctionnalité: Dictionnaire des prénoms et des noms de famille
  Dans un texte libre, un prénom ou un nom de famille connu du dictionnaire français est reconnu sans civilité,
  sans tenir compte de la casse ni des accents. Les mots courants qui sont aussi des noms ne sont pris pour des noms
  que lorsqu'ils portent une majuscule, et un nom inconnu du dictionnaire se trahit encore par la civilité qui le précède.

  Contexte:
    Étant donné un répertoire de données neuf

  Règle: Le dictionnaire reconnaît un nom sans tenir compte de la casse ni des accents

    Plan du Scénario: "<nom>" est reconnu comme <type> dans "<texte>"
      Quand l'agent lit la valeur "<texte>" dans la colonne "commentaire"
      Alors la colonne est signalée "<type>" pour la raison "detected"
      Et la valeur rendue ne contient plus "<nom>"
      Et la valeur rendue contient "colis remis à"

      Exemples:
        | texte                          | nom     | type       |
        | colis remis à DURAND ce matin  | DURAND  | last_name  |
        | colis remis à durand ce matin  | durand  | last_name  |
        | colis remis à Lefèvre ce matin | Lefèvre | last_name  |
        | colis remis à Zoé ce matin     | Zoé     | first_name |
        | colis remis à zoe ce matin     | zoe     | first_name |

  Règle: Un mot courant qui est aussi un nom n'est pris pour un nom qu'avec une majuscule

    Plan du Scénario: "<mot>" en minuscules reste un mot, "<Mot>" avec une majuscule est un nom
      Quand l'agent lit la valeur "un <mot> colis pour la maison" dans la colonne "commentaire"
      Alors la valeur reste en clair
      Quand l'agent lit la valeur "colis remis à <Mot> ce matin" dans la colonne "commentaire"
      Alors la valeur rendue ne contient plus "<Mot>"

      Exemples:
        | mot   | Mot   |
        | petit | Petit |
        | grand | Grand |
        | beau  | Beau  |

  Règle: Un nom composé est reconnu par ses parties

    Scénario: Un prénom composé est reconnu comme prénom
      Quand l'agent lit la valeur "colis remis à Jean-Luc ce matin" dans la colonne "commentaire"
      Alors la valeur rendue ne contient plus "Jean-Luc"
      Et la valeur rendue contient "colis remis à"

  Règle: Un prénom suivi de mots en majuscule forme un nom complet, jusqu'à trois mots

    Scénario: Un prénom connu entraîne les deux mots capitalisés qui le suivent
      Quand l'agent lit la valeur "colis remis à Marie Ylwenna Kervadec ce matin" dans la colonne "commentaire"
      Alors la colonne est signalée "person_name" pour la raison "detected"
      Et la valeur rendue ne contient plus "Ylwenna"
      Et la valeur rendue ne contient plus "Kervadec"

  Règle: Les mots trop courts et les particules ne sont jamais des noms

    Plan du Scénario: "<mot>" n'est pas pris pour un nom
      Quand l'agent lit la valeur "colis remis à <mot> ce matin" dans la colonne "commentaire"
      Alors la valeur reste en clair

      Exemples:
        | mot |
        | Le  |
        | Ly  |
        | De  |

  Règle: Une civilité révèle un nom absent du dictionnaire

    Plan du Scénario: Après "<civilité>", "<noms>" est pris pour un <type>
      Quand l'agent lit la valeur "colis remis à <civilité> <noms> ce matin" dans la colonne "commentaire"
      Alors la colonne est signalée "<type>" pour la raison "detected"
      Et la valeur rendue ne contient plus "<noms>"
      Et la valeur rendue contient "<civilité> "

      Exemples:
        | civilité | noms                        | type        |
        | M.       | Brizolier                   | last_name   |
        | Mme      | Ylwenna Kervadec            | person_name |
        | Monsieur | Odrigan Hazvekmor           | person_name |
        | Dr       | Maëlouz Le Brizolier        | person_name |
