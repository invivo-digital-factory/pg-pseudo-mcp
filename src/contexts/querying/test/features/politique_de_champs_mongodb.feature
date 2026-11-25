# language: fr
@querying
Fonctionnalité: Politique de champs MongoDB
  Le développeur corrige un faux positif des heuristiques sur un champ MongoDB, ou force la pseudonymisation d'un champ, en le déclarant
  dans la section [mongodb] des mêmes fichiers que la politique de colonnes. Une clé est "base.collection", où * remplace la base ou la collection ;
  une valeur est un chemin en notation pointée, où * remplace exactement un segment pour les documents qui utilisent des données comme clés.
  Les règles propres à PostgreSQL sont dans la politique de colonnes ; ce fichier tient à part parce que clés, chemins et jokers n'y ont pas d'équivalent.

  Règle: Une clé désigne une base et une collection, coupées au premier point
    Les noms de base et de collection se comparent exactement ; le chemin se compare entier et sans égard à la casse.

    Scénario: Un champ déclaré en clair pour toutes les bases est en clair et sans indice
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "*.products" = ["name"]
        """
      Quand la politique de colonnes est chargée
      Alors le champ "name" de "shop.products" est en clair
      Et le champ "NAME" de "crm.products" est en clair
      Et le champ "name" de "shop.Products" n'est pas classé
      Et le champ "name" de "shop.products" pseudonymisé comme "last_name" par le dictionnaire ne reçoit aucun indice

    Scénario: Une clé coupée au premier point désigne une collection dont le nom contient un point
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.orders.archive" = ["status"]
        """
      Quand la politique de colonnes est chargée
      Alors le champ "status" de "shop.orders.archive" est en clair
      Et le champ "status" de "shop.orders" n'est pas classé

    Scénario: Un chemin déclaré ne couvre ni ses enfants ni son parent
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.customers" = ["contact"]
        """
      Quand la politique de colonnes est chargée
      Alors le champ "contact" de "shop.customers" est en clair
      Et le champ "contact.email" de "shop.customers" n'est pas classé

  Règle: La clé de collection la plus précise l'emporte, puis le chemin le plus précis
    "base.collection", puis "*.collection", puis "base.*", puis "*.*" ; sous la même clé, le chemin qui a le plus de segments nommés,
    et à précision égale personal l'emporte sur clear.

    Scénario: La clé la plus précise l'emporte
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "*.*" = ["name"]

        [mongodb.personal."shop.orders"]
        "name" = "person_name"

        [mongodb.personal."*.orders"]
        "name" = "last_name"

        [mongodb.personal."shop.*"]
        "name" = "phone"
        """
      Quand la politique de colonnes est chargée
      Alors le champ "name" de "shop.orders" est personnel de type "person_name"
      Et le champ "name" de "crm.orders" est personnel de type "last_name"
      Et le champ "name" de "shop.customers" est personnel de type "phone"
      Et le champ "name" de "crm.customers" est en clair

    Scénario: Un chemin exact l'emporte sur un joker et le personnel l'emporte à précision égale
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.attributes" = ["select_options.*.label", "data.*.title"]

        [mongodb.personal."shop.attributes"]
        "select_options.vip.label" = "last_name"
        "data.red.*" = "person_name"
        """
      Quand la politique de colonnes est chargée
      Alors le champ "select_options.vip.label" de "shop.attributes" est personnel de type "last_name"
      Et le champ "select_options.red.label" de "shop.attributes" est en clair
      Et le champ "data.red.title" de "shop.attributes" est personnel de type "person_name"
      Et le champ "data.blue.title" de "shop.attributes" est en clair

    Scénario: La clé de collection la plus précise passe avant la précision du chemin
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.attributes" = ["select_options.*.label"]

        [mongodb.personal."*.attributes"]
        "select_options.red.label" = "last_name"
        """
      Quand la politique de colonnes est chargée
      Alors le champ "select_options.red.label" de "shop.attributes" est en clair
      Et le champ "select_options.red.label" de "crm.attributes" est personnel de type "last_name"

    Scénario: Le fichier local l'emporte sur le fichier partagé pour le même champ
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.orders" = ["status"]
        """
      Et la politique de colonnes locale suivante :
        """
        [mongodb.personal."shop.orders"]
        "status" = "person_name"
        """
      Quand la politique de colonnes est chargée
      Alors le champ "status" de "shop.orders" est personnel de type "person_name"

  Règle: Un joker couvre exactement un segment de chemin, quel que soit son nom
    Il ne couvre ni le parent, ni le sous-arbre, ni la racine du document ; un champ couvert par un joker ne reçoit pas d'indice.

    Scénario: Un joker couvre un seul segment de chemin, quel que soit son nom
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "shop.attributes" = ["select_options.*.label"]

        [mongodb.personal."shop.attributes"]
        "select_options.*.owner" = "person_name"
        """
      Quand la politique de colonnes est chargée
      Alors le champ "select_options.red.label" de "shop.attributes" est en clair
      Et le champ "SELECT_OPTIONS.A12.Label" de "shop.attributes" est en clair
      Et le champ "select_options.red.owner" de "shop.attributes" est personnel de type "person_name"
      Et le champ "select_options.red" de "shop.attributes" n'est pas classé
      Et le champ "select_options.red.label.fr" de "shop.attributes" n'est pas classé
      Et le champ "select_options.label" de "shop.attributes" n'est pas classé
      Et le champ "select_options.red.label" de "shop.attributes" pseudonymisé comme "last_name" par le dictionnaire ne reçoit aucun indice

  Règle: Sans entrée, un champ pris pour un nom par le dictionnaire reçoit un indice

    Scénario: Sans entrée, un champ pris pour un nom par le dictionnaire reçoit un indice
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]
        """
      Quand la politique de colonnes est chargée
      Alors le champ "name" de "shop.products" n'est pas classé
      Et le champ "name" de "shop.products" pseudonymisé comme "last_name" par le dictionnaire reçoit l'indice de collection "shop.products", de champ "name" et de raison "dictionary"

  Règle: Une section [mongodb] invalide fait ignorer tout le fichier, entrées PostgreSQL comprises
    Le fichier est signalé par column_policy_ignored ; un fichier valide garde côte à côte ses entrées PostgreSQL et MongoDB.

    Scénario: Un fichier valide garde ses entrées PostgreSQL à côté de ses entrées MongoDB
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]

        [mongodb.clear]
        "*.products" = ["name"]
        """
      Quand la politique de colonnes est chargée
      Alors aucun avertissement de politique n'est signalé
      Et la colonne "public.products.product_name" est en clair
      Et le champ "name" de "shop.products" est en clair

    Scénario: Le même champ déclaré en clair et personnel fait ignorer le fichier
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]

        [mongodb.clear]
        "shop.orders" = ["status"]

        [mongodb.personal."shop.orders"]
        "status" = "person_name"
        """
      Quand la politique de colonnes est chargée
      Alors un avertissement "column_policy_ignored" nomme le fichier partagé
      Et le champ "status" de "shop.orders" n'est pas classé
      Et la colonne "public.products.product_name" n'est pas classée

    Plan du Scénario: Le fichier est ignoré quand <défaut>
      Étant donné la politique de colonnes partagée suivante :
        """
        [mongodb.clear]
        "*.products" = ["name"]

        <section>
        """
      Quand la politique de colonnes est chargée
      Alors un avertissement "column_policy_ignored" nomme le fichier partagé
      Et le champ "name" de "shop.products" n'est pas classé
      Et le champ "<champ>" de "<collection>" n'est pas classé

      Exemples:
        | défaut                                  | champ                     | collection      | section                                                                          |
        | un type personnel est inconnu           | contact.mobile            | shop.customers  | [mongodb.personal."shop.customers"]\n"contact.mobile" = "telephone_portable"     |
        | un joker est mêlé à d'autres caractères | select_options.opt1.label | shop.attributes | [mongodb.personal."shop.attributes"]\n"select_options.opt*.label" = "last_name" |
        | un double joker est utilisé             | data.red.label            | shop.attributes | [mongodb.personal."shop.attributes"]\n"data.**" = "person_name"                  |
        | un joker mêlé est déclaré en clair      | select_options.opt1.label | shop.attributes | [mongodb.clear]\n"shop.attributes" = ["select_options.opt*.label"]              |
