function spiedPubSub() {

   var realPubSub = pubSub();

   function fakedPubSub( eventName ) {
        
      var single = realPubSub(eventName);

      var alreadySpied = !!single.emit.isSpy;
      
      if( !alreadySpied ) {   
         spyOn( single, 'emit' ).andCallThrough();
         spyOn( single, 'on'   ).andCallThrough();
         spyOn( single, 'un'   ).andCallThrough();
      }
      
      return single;
   }
   
   fakedPubSub.emit = realPubSub.emit;
   fakedPubSub.on = realPubSub.on;
   fakedPubSub.un = realPubSub.un;
   
   return fakedPubSub;
}

function fakePubSub( eventNames ) {

   var eventTypes = {};
   var eventsEmitted = [];   
   var eventNamesEmitted = [];
   var eventTypesEmitted = {};
   var callCount = {};
      
   function record(eventName){
      return function() {
         
         var args = Array.prototype.slice.apply(arguments);
         
         eventsEmitted.push({
            type: eventName, 
            args: args
         });
         
         eventNamesEmitted.push(eventName);
         eventTypesEmitted[eventName].push(args);
         callCount[eventName]++;
      }
   }      
   
   eventNames.forEach( function( eventName ){
      eventTypes[eventName] = {
         'emit':  jasmine.createSpy(eventName + '/emit')
                     .andCallFake(record(eventName))
      ,  'on':    jasmine.createSpy(eventName + '/on')
      ,  'un':    jasmine.createSpy(eventName + '/un')
      };
      
      eventTypesEmitted[eventName] = [];
      callCount[eventName] = 0;      
   });

   function bus( eventName ) {
           
      return eventTypes[eventName];
   }
   
   bus.events            = eventsEmitted;
   bus.eventNames        = eventNamesEmitted;
   bus.eventTypesEmitted = eventTypesEmitted;
   bus.callCount         = callCount;
   

   ['emit', 'on'].forEach(function(methodName){
   
      bus[methodName] = varArgs(function(eventName, parameters){
         apply( parameters, eventTypes[eventName][methodName]);
      });
   });

   return bus;
}

function eventBlackBox( pubsub, eventNames ) {
   
   var recording = [];
   
   eventNames.forEach(function(eventName){
      pubsub(eventName).on(function(val, val2){
         recording.push({
            type: eventName, 
            values: arguments,
            val: val,
            val2: val
         });
      });
   });
   
   return recording;
}
