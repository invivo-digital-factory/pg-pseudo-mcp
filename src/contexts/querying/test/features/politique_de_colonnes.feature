# language: fr
@integration @querying
Fonctionnalité: Politique de colonnes
  L'équipe corrige les faux positifs des heuristiques en déclarant des colonnes en clair,
  et force la pseudonymisation d'une colonne qu'elle sait personnelle.

  Scénario: Une colonne déclarée en clair rend sa valeur en clair
    Étant donné la politique de colonnes partagée suivante :
      """
      clear = ["*.product_name"]
      """
    Et l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 1"
    Alors la requête réussit
    Et la colonne "product_name" vaut "Garance Bleue" en clair

  Scénario: Une adresse e-mail reste remplacée dans une colonne déclarée en clair
    Étant donné la politique de colonnes partagée suivante :
      """
      clear = ["*.product_name"]
      """
    Et l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT product_name FROM products WHERE id = 2"
    Alors la requête réussit
    Et la colonne "product_name" ne laisse voir aucune adresse e-mail réelle

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

  Scénario: Une colonne déclarée en clair nourrit la lignée des expressions
    Étant donné la politique de colonnes partagée suivante :
      """
      clear = ["*.product_name"]
      """
    Et l'agent connecté à la base de test
    Quand l'agent exécute la requête "SELECT concat(product_name, ' ', status) AS v FROM products WHERE id = 1"
    Alors la requête réussit
    Et la colonne "v" vaut "Garance Bleue paid" en clair
