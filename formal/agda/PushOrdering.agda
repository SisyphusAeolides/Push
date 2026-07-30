{-# OPTIONS --safe --without-K #-}

module PushOrdering where

data Empty : Set where

Not : Set -> Set
Not proposition = proposition -> Empty

data Stage : Set where
  kernel packageManager messageBus compositor greeter session : Stage

data Step : Stage -> Stage -> Set where
  startCorinth : Step kernel packageManager
  startBus : Step packageManager messageBus
  startCompositor : Step messageBus compositor
  startGreeter : Step compositor greeter
  startSession : Step greeter session

kernelCannotSkipToSession : Not (Step kernel session)
kernelCannotSkipToSession ()

greeterRequiresCompositorStage : Not (Step messageBus greeter)
greeterRequiresCompositorStage ()

sessionRequiresGreeterStage : Not (Step compositor session)
sessionRequiresGreeterStage ()
