# language: fr
@querying
Fonctionnalité: Politique de champs MongoDB
  Le développeur corrige un faux positif des heuristiques sur un champ MongoDB en le déclarant
  dans la section [mongodb] du fichier de politique de colonnes.

  Scénario: Un champ déclaré en clair pour toutes les bases est en clair et sans indice
    Étant donné la politique de colonnes partagée suivante :
      """
      [mongodb.clear]
      "*.products" = ["name"]
      """
    Quand la politique de colonnes est chargée
    Alors le champ "name" de "shop.products" est en clair
    Et le champ "name" de "shop.products" pseudonymisé comme "last_name" par le dictionnaire ne reçoit aucun indice

  Scénario: Sans entrée, un champ pris pour un nom par le dictionnaire reçoit un indice
    Étant donné la politique de colonnes partagée suivante :
      """
      clear = ["*.product_name"]
      """
    Quand la politique de colonnes est chargée
    Alors le champ "name" de "shop.products" n'est pas classé
    Et le champ "name" de "shop.products" pseudonymisé comme "last_name" par le dictionnaire reçoit l'indice de collection "shop.products", de champ "name" et de raison "dictionary"

  Scénario: Une clé coupée au premier point désigne une collection dont le nom contient un point
    Étant donné la politique de colonnes partagée suivante :
      """
      [mongodb.clear]
      "shop.orders.archive" = ["status"]
      """
    Quand la politique de colonnes est chargée
    Alors le champ "status" de "shop.orders.archive" est en clair
    Et le champ "status" de "shop.orders" n'est pas classé

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

  Scénario: Un type inconnu dans la section personnelle fait ignorer le fichier
    Étant donné la politique de colonnes partagée suivante :
      """
      [mongodb.personal."shop.customers"]
      "contact.mobile" = "telephone_portable"
      """
    Quand la politique de colonnes est chargée
    Alors un avertissement "column_policy_ignored" nomme le fichier partagé
    Et le champ "contact.mobile" de "shop.customers" n'est pas classé

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
