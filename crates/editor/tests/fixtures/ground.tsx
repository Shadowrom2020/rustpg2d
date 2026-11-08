<?xml version="1.0" encoding="UTF-8"?>
<tileset version="1.10" name="ground" tilewidth="16" tileheight="16" tilecount="4" columns="2">
 <image source="ground16.png" width="32" height="32"/>
 <tile id="1">
  <animation>
   <frame tileid="1" duration="250"/>
   <frame tileid="2" duration="250"/>
  </animation>
 </tile>
 <tile id="3">
  <properties>
   <property name="solid" type="bool" value="true"/>
  </properties>
 </tile>
 <wangsets>
  <wangset name="Paths" type="mixed" tile="0">
   <wangcolor name="Path" color="#c8a050" tile="0" probability="1"/>
   <wangtile tileid="0" wangid="1,1,1,1,1,1,1,1"/>
   <wangtile tileid="1" wangid="1,0,0,0,0,0,0,0"/>
   <wangtile tileid="2" wangid="1,0,1,0,0,0,0,0"/>
   <wangtile tileid="3" wangid="0,1,0,0,0,0,0,0"/>
  </wangset>
  <wangset name="Grass corners" type="corner" tile="-1">
   <wangcolor name="Grass" color="#00ff00" tile="-1" probability="1"/>
   <wangtile tileid="0" wangid="0,1,0,1,0,1,0,1"/>
  </wangset>
 </wangsets>
</tileset>
