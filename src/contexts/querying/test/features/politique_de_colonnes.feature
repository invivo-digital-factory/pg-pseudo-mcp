# language: fr
@querying
Fonctionnalité: Politique de colonnes
  L'équipe corrige les faux positifs des heuristiques en déclarant des colonnes en clair, et force la pseudonymisation d'une colonne qu'elle sait personnelle.
  Deux fichiers TOML sont relus à chaque appel : le fichier partagé de l'équipe, puis le fichier local du développeur, qui l'emporte sur la même clé.
  Une clé est schema.table.colonne, table.colonne ou *.colonne ; la plus précise l'emporte. Les champs MongoDB ont leur propre section, voir la politique de champs.

  Règle: Une colonne déclarée en clair rend sa valeur en clair, sauf ce que les détecteurs structurels reconnaissent
    Un clear saute le dictionnaire et les heuristiques de noms, mais une adresse e-mail, un téléphone, un IBAN ou un SIRET restent remplacés.

    @integration
    Scénario: Une colonne déclarée en clair rend sa valeur en clair
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "product_name" vaut "Garance Bleue" en clair

    @integration
    Scénario: Une adresse e-mail reste remplacée dans une colonne déclarée en clair
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 2"
      Alors la requête réussit
      Et la colonne "product_name" ne laisse voir aucune adresse e-mail réelle

    @integration
    Scénario: Une colonne déclarée en clair nourrit la lignée des expressions
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT concat(product_name, ' ', status) AS v FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "v" vaut "Garance Bleue paid" en clair

    @integration
    Scénario: Une colonne calculée n'est jamais confondue avec une colonne déclarée par son alias
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT upper(nom) AS product_name FROM customers WHERE id = 1"
      Alors la requête réussit
      Et la réponse ne contient aucune valeur canari

  Règle: Une colonne déclarée personnelle est pseudonymisée avec le type donné

    @integration
    Scénario: Une colonne déclarée personnelle est pseudonymisée avec le type donné
      Étant donné la politique de colonnes partagée suivante :
        """
        [personal]
        "products.supplier_ref" = "last_name"
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT supplier_ref FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "supplier_ref" est pseudonymisée comme "last_name" par la politique

  Règle: La clé la plus précise l'emporte, puis le fichier local sur le fichier partagé
    schema.table.colonne, puis table.colonne, puis *.colonne ; les noms se comparent sans égard à la casse.

    @integration
    Scénario: Schéma, table et colonne l'emportent sur l'étoile
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]

        [personal]
        "public.products.product_name" = "last_name"
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "product_name" est pseudonymisée comme "last_name" par la politique

    @integration
    Scénario: Le fichier local l'emporte sur le fichier partagé pour la même colonne
      Étant donné la politique de colonnes partagée suivante :
        """
        [personal]
        "*.product_name" = "last_name"
        """
      Et la politique de colonnes locale suivante :
        """
        clear = ["*.product_name"]
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "product_name" vaut "Garance Bleue" en clair

    Scénario: Chaque forme de clé couvre sa portée
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.Product_Name", "public.catalog.product_name"]

        [personal]
        "catalog.product_name" = "last_name"
        """
      Quand la politique de colonnes est chargée
      Alors la colonne "public.catalog.product_name" est en clair
      Et la colonne "sales.catalog.product_name" est personnelle de type "last_name"
      Et la colonne "sales.orders.PRODUCT_NAME" est en clair
      Et la colonne "sales.orders.label" n'est pas classée

  Règle: Un fichier invalide est ignoré en entier et signalé, sans arrêter le serveur
    Un fichier illisible, une clé mal formée, un type inconnu ou une colonne à la fois claire et personnelle : le fichier est ignoré, l'autre fichier s'applique,
    et list_connections comme server_info portent column_policy_ignored en nommant le fichier.

    @integration
    Scénario: Un fichier de politique invalide est signalé sans changer le comportement
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]

        [personal]
        "*.supplier_ref" = "numero_fiscal"
        """
      Et l'agent connecté à la base de test
      Quand l'agent liste les connexions
      Alors la liste signale que la politique partagée est ignorée
      Quand l'agent exécute la requête "SELECT supplier_ref FROM products WHERE id = 1"
      Alors la requête réussit
      Et la colonne "supplier_ref" vaut "lot-42" en clair

    Plan du Scénario: Le fichier est ignoré quand <défaut>
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name", <clé claire>]

        [personal]
        <clé personnelle> = "<type>"
        """
      Quand la politique de colonnes est chargée
      Alors un avertissement "column_policy_ignored" nomme le fichier partagé
      Et la colonne "public.products.product_name" n'est pas classée

      Exemples:
        | défaut                                         | clé claire         | clé personnelle    | type          |
        | un type est inconnu                            | "orders.reference" | "*.supplier_ref"   | numero_fiscal |
        | une clé n'a pas de table                       | "product_name"     | "*.supplier_ref"   | siret         |
        | une clé a trop de segments                     | "a.b.c.d"          | "*.supplier_ref"   | siret         |
        | une étoile remplace la table                   | "orders.*"         | "*.supplier_ref"   | siret         |
        | une colonne est à la fois claire et personnelle | "orders.reference" | "orders.reference" | siret         |

    Scénario: Le fichier local valide s'applique quand le fichier partagé est ignoré
      Étant donné la politique de colonnes partagée suivante :
        """
        [personal]
        "*.supplier_ref" = "numero_fiscal"
        """
      Et la politique de colonnes locale suivante :
        """
        clear = ["*.product_name"]
        """
      Quand la politique de colonnes est chargée
      Alors un avertissement "column_policy_ignored" nomme le fichier partagé
      Et la colonne "public.products.product_name" est en clair

  @integration
  Règle: Une colonne pseudonymisée par défaut ou prise pour un nom porte un indice tant qu'elle n'est pas classée
    L'indice propose de la classer clear ou personal ; une colonne que le dictionnaire reconnaît comme e-mail, téléphone ou ville n'en reçoit pas.

    Scénario: Une colonne non classée porte un indice qui disparaît une fois classée
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 1"
      Alors la réponse propose de classer la colonne "public.products.product_name"
      Quand l'agent décrit la table "products"
      Alors la réponse propose de classer la colonne "public.products.supplier_ref"

    Scénario: Une colonne classée ne porte plus d'indice
      Étant donné la politique de colonnes partagée suivante :
        """
        clear = ["*.product_name"]

        [personal]
        "products.supplier_ref" = "siret"
        """
      Et l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT product_name, supplier_ref FROM products WHERE id = 1"
      Alors la réponse ne propose de classer aucune colonne
      Quand l'agent décrit la table "products"
      Alors la réponse ne propose de classer aucune colonne

    Plan du Scénario: Une colonne <colonne> reconnue par le dictionnaire comme <type> <indice>
      Étant donné l'agent connecté à la base de test
      Quand l'agent exécute la requête "SELECT <colonne> FROM customers WHERE id = 1"
      Alors la colonne "<colonne>" est pseudonymisée comme "<type>" par le dictionnaire
      Et la réponse <indice>

      Exemples:
        | colonne   | type       | indice                                             |
        | nom       | last_name  | propose de classer la colonne "public.customers.nom" |
        | prenom    | first_name | propose de classer la colonne "public.customers.prenom" |
        | email     | email      | ne propose de classer aucune colonne               |
        | telephone | phone      | ne propose de classer aucune colonne               |
        | ville     | city       | ne propose de classer aucune colonne               |
